use std::ffi::c_void;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use windows::core::w;
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{
    RegisterSuspendResumeNotification, UnregisterSuspendResumeNotification,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, PostMessageW, PostQuitMessage, RegisterClassW, SetWindowLongPtrW,
    TranslateMessage, CREATESTRUCTW, DEVICE_NOTIFY_WINDOW_HANDLE, GWLP_USERDATA, HWND_MESSAGE, MSG,
    PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMECRITICAL, PBT_APMRESUMESTANDBY, PBT_APMRESUMESUSPEND,
    PBT_APMSUSPEND, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_CREATE, WM_DESTROY, WM_NCCREATE,
    WM_NCDESTROY, WM_POWERBROADCAST, WNDCLASSW,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSignal {
    Suspend,
    Resume,
}

pub struct PowerMonitor {
    window: isize,
    thread: Option<JoinHandle<()>>,
}

impl PowerMonitor {
    pub fn spawn(sender: UnboundedSender<PowerSignal>) -> Result<Self, String> {
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("switchify-power-events".into())
            .spawn(move || run_message_window(sender, ready_sender))
            .map_err(|error| error.to_string())?;
        let window = ready_receiver
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "The Windows power monitor did not start.".to_string())??;
        Ok(Self {
            window,
            thread: Some(thread),
        })
    }
}

impl Drop for PowerMonitor {
    fn drop(&mut self) {
        if self.window != 0 {
            let window = HWND(self.window as *mut c_void);
            unsafe {
                let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_message_window(
    sender: UnboundedSender<PowerSignal>,
    ready: mpsc::SyncSender<Result<isize, String>>,
) {
    let result = unsafe {
        let instance = match GetModuleHandleW(None) {
            Ok(instance) => instance,
            Err(error) => {
                let _ = ready.send(Err(error.to_string()));
                return;
            }
        };
        let class = w!("SwitchifyPowerMonitorWindow");
        let registration = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        let _ = RegisterClassW(&registration);
        let context = Box::into_raw(Box::new(sender));
        match CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!(""),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance.into()),
            Some(context.cast()),
        ) {
            Ok(window) => Ok((window, context)),
            Err(error) => Err(error.to_string()),
        }
    };
    let (window, _) = match result {
        Ok(value) => value,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let notification =
        unsafe { RegisterSuspendResumeNotification(HANDLE(window.0), DEVICE_NOTIFY_WINDOW_HANDLE) };
    let notification = match notification {
        Ok(notification) => notification,
        Err(error) => {
            unsafe {
                let _ = DestroyWindow(window);
            }
            let _ = ready.send(Err(error.to_string()));
            return;
        }
    };
    let _ = ready.send(Ok(window.0 as isize));
    unsafe {
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = UnregisterSuspendResumeNotification(notification);
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        if message == WM_POWERBROADCAST {
            if let Some(signal) = map_power_message(wparam.0 as u32) {
                let context =
                    GetWindowLongPtrW(window, GWLP_USERDATA) as *const UnboundedSender<PowerSignal>;
                if let Some(sender) = context.as_ref() {
                    let _ = sender.send(signal);
                }
            }
            return LRESULT(1);
        }
        if message == WM_CLOSE {
            let _ = DestroyWindow(window);
            return LRESULT(0);
        }
        if message == WM_DESTROY {
            PostQuitMessage(0);
            return LRESULT(0);
        }
        if message == WM_NCDESTROY {
            let context =
                GetWindowLongPtrW(window, GWLP_USERDATA) as *mut UnboundedSender<PowerSignal>;
            SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            if !context.is_null() {
                drop(Box::from_raw(context));
            }
        }
        if message == WM_CREATE {
            return LRESULT(0);
        }
        DefWindowProcW(window, message, wparam, lparam)
    }
}

fn map_power_message(value: u32) -> Option<PowerSignal> {
    match value {
        PBT_APMSUSPEND => Some(PowerSignal::Suspend),
        PBT_APMRESUMEAUTOMATIC
        | PBT_APMRESUMESUSPEND
        | PBT_APMRESUMECRITICAL
        | PBT_APMRESUMESTANDBY => Some(PowerSignal::Resume),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_suspend_and_all_resume_power_messages() {
        assert_eq!(
            map_power_message(PBT_APMSUSPEND),
            Some(PowerSignal::Suspend)
        );
        for message in [
            PBT_APMRESUMEAUTOMATIC,
            PBT_APMRESUMESUSPEND,
            PBT_APMRESUMECRITICAL,
            PBT_APMRESUMESTANDBY,
        ] {
            assert_eq!(map_power_message(message), Some(PowerSignal::Resume));
        }
        assert_eq!(map_power_message(0), None);
    }
}
