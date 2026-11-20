//! Starts a worker copy of this executable and connects to it.
//!
//! Release builds carry a UIAccess manifest. Windows refuses to create a
//! UIAccess process with `CreateProcess` unless the caller is elevated
//! (`ERROR_ELEVATION_REQUIRED`), so on a standard account, or an
//! administrator account without elevation, a worker started that way
//! never runs. The Shell starts it through the AppInfo service instead,
//! which grants UIAccess without elevation.
//!
//! A Shell launch cannot hand the worker inherited pipes, so the worker
//! connects to two named pipes instead, one for each direction. Each pipe
//! has an unguessable name, one instance, refuses remote clients, only
//! lets the current user and SYSTEM open it, and is accepted only from the
//! process that was started. The worker accepts only pipes served by the
//! process named on its command line.
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io,
    os::windows::{
        ffi::OsStrExt,
        io::{FromRawHandle, RawHandle},
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, DuplicateHandle, GetLastError, LocalFree, DUPLICATE_SAME_ACCESS,
        ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING, GENERIC_READ, GENERIC_WRITE, HANDLE,
        INVALID_HANDLE_VALUE, STILL_ACTIVE, WAIT_OBJECT_0,
    },
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SDDL_REVISION_1,
        },
        GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
        TOKEN_USER,
    },
    Storage::FileSystem::{
        CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
        SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
    },
    System::{
        Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE},
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId,
            GetNamedPipeServerProcessId, SetNamedPipeHandleState, PIPE_NOWAIT, PIPE_READMODE_BYTE,
            PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
        },
        Threading::{
            GetCurrentProcess, GetCurrentProcessId, GetExitCodeProcess, GetProcessId,
            OpenProcessToken, TerminateProcess, WaitForSingleObject, INFINITE,
        },
    },
    UI::{
        Shell::{
            ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS,
            SEE_MASK_NOZONECHECKS, SEE_MASK_NO_CONSOLE, SHELLEXECUTEINFOW,
        },
        WindowsAndMessaging::SW_HIDE,
    },
};

/// Precedes the pipe name and the parent process id on a worker's command line.
pub const PIPE_ARG: &str = "--switchify-pipe";
const BUFFER: u32 = 64 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// A started worker process. Dropping it closes the handle but does not end
/// the process: the job it was placed in does that.
pub struct Worker {
    handle: HANDLE,
    id: u32,
}
unsafe impl Send for Worker {}
unsafe impl Sync for Worker {}
impl Worker {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn id(&self) -> u32 {
        self.id
    }
    /// The exit code once the worker has ended.
    pub fn try_wait(&self) -> io::Result<Option<u32>> {
        let mut code = 0;
        if unsafe { GetExitCodeProcess(self.handle, &mut code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok((code != STILL_ACTIVE as u32 || self.ended()).then_some(code))
    }
    fn ended(&self) -> bool {
        unsafe { WaitForSingleObject(self.handle, 0) == WAIT_OBJECT_0 }
    }
    pub fn kill(&self) -> io::Result<()> {
        if unsafe { TerminateProcess(self.handle, 1) } == 0 {
            let error = io::Error::last_os_error();
            if !self.ended() {
                return Err(error);
            }
        }
        Ok(())
    }
    /// A second handle to the same process, for a thread that waits on it
    /// while this one is used elsewhere.
    pub fn duplicate(&self) -> io::Result<Self> {
        let mut handle = std::ptr::null_mut();
        let current = unsafe { GetCurrentProcess() };
        if unsafe {
            DuplicateHandle(
                current,
                self.handle,
                current,
                &mut handle,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            handle,
            id: self.id,
        })
    }
    /// Kills a worker that will not be used, waiting a bounded time for it.
    fn end(&self) {
        let _ = self.kill();
        unsafe { WaitForSingleObject(self.handle, 5000) };
    }
    pub fn wait(&self) -> io::Result<()> {
        unsafe { WaitForSingleObject(self.handle, INFINITE) };
        Ok(())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
    }
}

/// The parent's ends of a worker's pipes.
pub struct Pipes {
    /// Requests to the worker.
    pub to_worker: File,
    /// Replies from the worker.
    pub from_worker: File,
}

struct Owned(HANDLE);
unsafe impl Send for Owned {}
impl Owned {
    fn into_file(self) -> File {
        let file = unsafe { File::from_raw_handle(self.0 as RawHandle) };
        std::mem::forget(self);
        file
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

/// One argument quoted for `CommandLineToArgvW` and the C runtime.
fn quote(arg: &OsStr, out: &mut Vec<u16>) {
    let units: Vec<u16> = arg.encode_wide().collect();
    let plain = !units.is_empty()
        && !units
            .iter()
            .any(|&u| u == ' ' as u16 || u == '\t' as u16 || u == '"' as u16);
    if plain {
        out.extend_from_slice(&units);
        return;
    }
    out.push('"' as u16);
    let mut backslashes = 0;
    for &unit in &units {
        if unit == '\\' as u16 {
            backslashes += 1;
            continue;
        }
        let repeat = if unit == '"' as u16 {
            backslashes * 2 + 1
        } else {
            backslashes
        };
        out.extend(std::iter::repeat_n('\\' as u16, repeat));
        backslashes = 0;
        out.push(unit);
    }
    out.extend(std::iter::repeat_n('\\' as u16, backslashes * 2));
    out.push('"' as u16);
}

fn command_line(args: &[OsString]) -> Vec<u16> {
    let mut out = Vec::new();
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            out.push(' ' as u16);
        }
        quote(arg, &mut out);
    }
    out.push(0);
    out
}

/// A security descriptor that lets only the current user and SYSTEM open
/// the pipe. The default one also lets everyone read, so another local
/// user could connect first and stop the worker from starting.
struct OwnerOnly(PSECURITY_DESCRIPTOR);
impl Drop for OwnerOnly {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0) };
    }
}
fn owner_only() -> io::Result<OwnerOnly> {
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = Owned(token);
        let mut size = 0;
        GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut size);
        // u64 storage keeps TOKEN_USER aligned.
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        if GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            size,
            &mut size,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut sid = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid) == 0 {
            return Err(io::Error::last_os_error());
        }
        let length = (0..).take_while(|&i| *sid.add(i) != 0).count();
        let sid_text = String::from_utf16_lossy(std::slice::from_raw_parts(sid, length));
        LocalFree(sid.cast());
        let sddl = wide(OsStr::new(&format!("D:P(A;;GA;;;{sid_text})(A;;GA;;;SY)")));
        let mut descriptor = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(OwnerOnly(descriptor))
    }
}

/// Duplex even though each pipe carries one direction: switching an inbound
/// pipe to blocking needs write access to its attributes.
fn create_pipe(name: &OsStr) -> io::Result<Owned> {
    let security = owner_only()?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.0,
        bInheritHandle: 0,
    };
    let name = wide(name);
    let handle = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            BUFFER,
            BUFFER,
            0,
            &attributes,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(Owned(handle))
}

/// Waits for the worker to connect, then switches the pipe to blocking.
fn accept(pipe: &Owned, worker: &Worker, deadline: Instant) -> io::Result<()> {
    loop {
        let connected = unsafe { ConnectNamedPipe(pipe.0, std::ptr::null_mut()) } != 0;
        let error = unsafe { GetLastError() };
        if connected || error == ERROR_PIPE_CONNECTED {
            break;
        }
        if error != ERROR_PIPE_LISTENING {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        if worker.ended() {
            return Err(io::Error::other("The worker ended before it connected."));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "The worker did not connect.",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut client = 0;
    if unsafe { GetNamedPipeClientProcessId(pipe.0, &mut client) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if client != worker.id {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Another process connected to the worker pipe.",
        ));
    }
    let mode = PIPE_READMODE_BYTE | PIPE_WAIT;
    if unsafe { SetNamedPipeHandleState(pipe.0, &mode, std::ptr::null(), std::ptr::null()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

struct Com(bool);
impl Com {
    fn new() -> Self {
        let result = unsafe {
            CoInitializeEx(
                std::ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            )
        };
        Self(result >= 0)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

fn shell_execute(executable: &OsStr, parameters: &[OsString]) -> io::Result<Worker> {
    let _com = Com::new();
    let file = wide(executable);
    let parameters = command_line(parameters);
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    // NO_CONSOLE: a debug (console) build's worker shares the app's console
    // instead of opening one. NOZONECHECKS: it is our own installed exe.
    info.fMask = SEE_MASK_NOCLOSEPROCESS
        | SEE_MASK_NOASYNC
        | SEE_MASK_FLAG_NO_UI
        | SEE_MASK_NO_CONSOLE
        | SEE_MASK_NOZONECHECKS;
    info.lpFile = file.as_ptr();
    info.lpParameters = parameters.as_ptr();
    info.nShow = SW_HIDE;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.hProcess.is_null() {
        return Err(io::Error::other(
            "The Shell did not return the worker process.",
        ));
    }
    let id = unsafe { GetProcessId(info.hProcess) };
    Ok(Worker {
        handle: info.hProcess,
        id,
    })
}

/// The parent's pipes before the worker has connected to them.
pub struct Pending {
    to_worker: Owned,
    from_worker: Owned,
}
impl Pending {
    /// Waits for `worker` to connect to both pipes, up to a few seconds,
    /// and gives up early if it ends. Nothing is read or written first.
    pub fn accept(self, worker: &Worker) -> io::Result<Pipes> {
        let deadline = Instant::now() + CONNECT_TIMEOUT;
        accept(&self.to_worker, worker, deadline)?;
        accept(&self.from_worker, worker, deadline)?;
        Ok(Pipes {
            to_worker: self.to_worker.into_file(),
            from_worker: self.from_worker.into_file(),
        })
    }
}

/// Starts this executable with `args`, then the pipe arguments, through the
/// Shell, without waiting for it to connect. `contain` runs on the new
/// process first, so a worker that cannot be placed in its job is ended
/// and never used.
pub fn start(
    args: &[OsString],
    contain: impl FnOnce(HANDLE) -> io::Result<()>,
) -> io::Result<(Worker, Pending)> {
    let executable = std::env::current_exe()?;
    let base = format!(r"\\.\pipe\switchify-worker-{}", uuid::Uuid::new_v4());
    let to_name = OsString::from(format!("{base}-in"));
    let from_name = OsString::from(format!("{base}-out"));
    let to_worker = create_pipe(&to_name)?;
    let from_worker = create_pipe(&from_name)?;
    let mut parameters = args.to_vec();
    parameters.extend([
        OsString::from(PIPE_ARG),
        OsString::from(base),
        OsString::from(unsafe { GetCurrentProcessId() }.to_string()),
    ]);
    let worker = shell_execute(executable.as_os_str(), &parameters)?;
    if let Err(error) = contain(worker.handle) {
        worker.end();
        return Err(error);
    }
    Ok((
        worker,
        Pending {
            to_worker,
            from_worker,
        },
    ))
}

/// [`start`], then waits for the worker to connect.
pub fn launch(
    args: &[OsString],
    contain: impl FnOnce(HANDLE) -> io::Result<()>,
) -> io::Result<(Worker, Pipes)> {
    let (worker, pending) = start(args, contain)?;
    match pending.accept(&worker) {
        Ok(pipes) => Ok((worker, pipes)),
        Err(error) => {
            worker.end();
            Err(error)
        }
    }
}

/// The worker's ends of its pipes: requests in, replies out.
pub struct WorkerPipes {
    pub input: File,
    pub output: File,
}

fn open(name: &OsStr, access: u32, parent: u32) -> io::Result<Owned> {
    let name = wide(name);
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            access,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let pipe = Owned(handle);
    let mut server = 0;
    if unsafe { GetNamedPipeServerProcessId(pipe.0, &mut server) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if server != parent {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "The worker pipe is not served by the app that started this worker.",
        ));
    }
    Ok(pipe)
}

/// Splits the pipe arguments from the end of a worker's command line and
/// connects to them. `None` when the command line has no pipe arguments.
pub fn connect_from_args(args: &[OsString]) -> Option<(usize, io::Result<WorkerPipes>)> {
    let at = args.len().checked_sub(3)?;
    if args[at] != PIPE_ARG {
        return None;
    }
    let connect = || -> io::Result<WorkerPipes> {
        let base = args[at + 1].to_str().ok_or(io::ErrorKind::InvalidInput)?;
        if !base.starts_with(r"\\.\pipe\switchify-worker-") {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let parent: u32 = args[at + 2]
            .to_str()
            .and_then(|pid| pid.parse().ok())
            .ok_or(io::ErrorKind::InvalidInput)?;
        let input = open(OsStr::new(&format!("{base}-in")), GENERIC_READ, parent)?;
        let output = open(OsStr::new(&format!("{base}-out")), GENERIC_WRITE, parent)?;
        Ok(WorkerPipes {
            input: input.into_file(),
            output: output.into_file(),
        })
    };
    Some((at, connect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quoted(arg: &str) -> String {
        let mut out = Vec::new();
        quote(OsStr::new(arg), &mut out);
        String::from_utf16(&out).unwrap()
    }

    #[test]
    fn arguments_are_quoted_for_the_c_runtime() {
        assert_eq!(quoted("--switchify-pipe"), "--switchify-pipe");
        assert_eq!(quoted("[32,13]"), "[32,13]");
        assert_eq!(quoted(""), "\"\"");
        assert_eq!(
            quoted(r"C:\Program Files\Switchify PC\resources\textslinger\runtime"),
            r#""C:\Program Files\Switchify PC\resources\textslinger\runtime""#
        );
        assert_eq!(quoted(r"C:\a b\"), r#""C:\a b\\""#);
        assert_eq!(quoted(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(quoted(r#"a\"b"#), r#""a\\\"b""#);
    }

    #[test]
    fn command_lines_join_quoted_arguments() {
        let line = command_line(&[OsString::from("a b"), OsString::from("c")]);
        assert_eq!(
            String::from_utf16(&line[..line.len() - 1]).unwrap(),
            "\"a b\" c"
        );
    }

    #[test]
    fn pipe_arguments_are_found_only_at_the_end() {
        let args = |list: &[&str]| list.iter().map(OsString::from).collect::<Vec<_>>();
        assert!(connect_from_args(&args(&["app", "--x"])).is_none());
        assert!(connect_from_args(&args(&["app", PIPE_ARG, "name", "1", "extra"])).is_none());
        let (at, result) =
            connect_from_args(&args(&["app", "--x", PIPE_ARG, r"\\.\pipe\other", "1"])).unwrap();
        assert_eq!(at, 2);
        assert!(result.is_err());
        let (_, result) = connect_from_args(&args(&[
            "app",
            PIPE_ARG,
            r"\\.\pipe\switchify-worker-x",
            "not-a-pid",
        ]))
        .unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn a_pipe_from_another_server_is_refused() {
        // A pipe this process serves, opened as if this process were a
        // worker started by some other parent.
        let pipe = || {
            let name = format!(r"\\.\pipe\switchify-worker-test-{}", uuid::Uuid::new_v4());
            let server = create_pipe(OsStr::new(&name)).unwrap();
            (name, server)
        };
        let me = unsafe { GetCurrentProcessId() };
        let (name, _server) = pipe();
        let error = open(OsStr::new(&name), GENERIC_READ, me + 1).err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        let (name, _server) = pipe();
        assert!(open(OsStr::new(&name), GENERIC_READ, me).is_ok());
    }
}
