use std::ffi::c_void;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use windows::core::{w, GUID};
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{
    RegisterPowerSettingNotification, RegisterSuspendResumeNotification,
    UnregisterPowerSettingNotification, UnregisterSuspendResumeNotification,
    POWERBROADCAST_SETTING,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, PostMessageW, PostQuitMessage, RegisterClassW, SetWindowLongPtrW,
    TranslateMessage, CREATESTRUCTW, DEVICE_NOTIFY_WINDOW_HANDLE, GWLP_USERDATA, HWND_MESSAGE, MSG,
    PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMECRITICAL, PBT_APMRESUMESTANDBY, PBT_APMRESUMESUSPEND,
    PBT_APMSUSPEND, PBT_POWERSETTINGCHANGE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_CREATE,
    WM_DESTROY, WM_NCCREATE, WM_NCDESTROY, WM_POWERBROADCAST, WNDCLASSW,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSignal {
    Suspend,
    ResumeAutomatic,
    ResumeInteractive,
    DisplayOn,
}

// Interactive applications must use the session display setting, including
// on Modern Standby PCs which need not send the classic wake broadcasts.
const SESSION_DISPLAY_STATUS: GUID = GUID::from_u128(0x2b84c20e_ad23_4ddf_93db_05ffbd7efca5);

#[derive(Default)]
struct DisplayPower {
    off: bool,
}

impl DisplayPower {
    fn accept(&mut self, setting: GUID, data: &[u8]) -> Option<PowerSignal> {
        if setting != SESSION_DISPLAY_STATUS || data.len() != 4 {
            return None;
        }
        match u32::from_le_bytes(data.try_into().ok()?) {
            0 => self.off = true,
            1 | 2 => {
                if std::mem::take(&mut self.off) {
                    return Some(PowerSignal::DisplayOn);
                }
            }
            _ => {}
        }
        None
    }
}

struct WindowContext {
    sender: UnboundedSender<PowerSignal>,
    display: DisplayPower,
}

/// Windows sends automatic resume for every wake, including unattended
/// wakes. Interactive resume follows when the user returns, possibly much
/// later. Keep that pair in one cycle instead of using a time window.
#[derive(Default)]
pub struct PowerCycle {
    resumed: bool,
}

impl PowerCycle {
    pub fn accept(&mut self, signal: PowerSignal) -> bool {
        match signal {
            PowerSignal::Suspend => {
                self.resumed = false;
                true
            }
            PowerSignal::ResumeAutomatic => {
                // Automatic resume starts a new wake even if Windows did
                // not deliver the preceding suspend notification.
                self.resumed = true;
                true
            }
            PowerSignal::ResumeInteractive => {
                let first = !self.resumed;
                self.resumed = true;
                first
            }
            PowerSignal::DisplayOn => {
                // A worker started on an earlier unattended wake can fail
                // before the display returns. Recover again at that boundary.
                self.resumed = true;
                true
            }
        }
    }
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
        let context = Box::into_raw(Box::new(WindowContext {
            sender,
            display: DisplayPower::default(),
        }));
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
    let display_notification = unsafe {
        RegisterPowerSettingNotification(
            HANDLE(window.0),
            &SESSION_DISPLAY_STATUS,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )
    };
    let display_notification = match display_notification {
        Ok(notification) => notification,
        Err(error) => {
            unsafe {
                let _ = UnregisterSuspendResumeNotification(notification);
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
        let _ = UnregisterPowerSettingNotification(display_notification);
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
            let context = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut WindowContext;
            if let Some(context) = context.as_mut() {
                let signal = if wparam.0 as u32 == PBT_POWERSETTINGCHANGE && lparam.0 != 0 {
                    let setting = &*(lparam.0 as *const POWERBROADCAST_SETTING);
                    if setting.DataLength == 4 {
                        let data = std::slice::from_raw_parts(setting.Data.as_ptr(), 4);
                        context.display.accept(setting.PowerSetting, data)
                    } else {
                        None
                    }
                } else {
                    map_power_message(wparam.0 as u32)
                };
                if let Some(signal) = signal {
                    let _ = context.sender.send(signal);
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
            let context = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut WindowContext;
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
        PBT_APMRESUMEAUTOMATIC => Some(PowerSignal::ResumeAutomatic),
        PBT_APMRESUMESUSPEND | PBT_APMRESUMECRITICAL | PBT_APMRESUMESTANDBY => {
            Some(PowerSignal::ResumeInteractive)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_return_recovers_without_classic_wake_and_ignores_duplicates() {
        let mut display = DisplayPower::default();
        let mut cycle = PowerCycle::default();
        let mut accept = |value: u32| display.accept(SESSION_DISPLAY_STATUS, &value.to_le_bytes());
        // Registration reports the initial state: do not reset live input.
        assert_eq!(accept(1), None);
        assert_eq!(accept(2), None);
        assert_eq!(accept(0), None);
        let signal = accept(1).unwrap();
        assert!(cycle.accept(signal));
        assert!(!cycle.accept(PowerSignal::ResumeInteractive));
        assert_eq!(accept(1), None);
        assert_eq!(accept(2), None);
        assert_eq!(accept(0), None);
        assert_eq!(accept(0), None);
        assert_eq!(accept(2), Some(PowerSignal::DisplayOn));
    }

    #[test]
    fn display_return_retires_worker_started_by_unattended_hibernation_wake() {
        let mut display = DisplayPower::default();
        let mut cycle = PowerCycle::default();
        assert_eq!(
            display.accept(SESSION_DISPLAY_STATUS, &0u32.to_le_bytes()),
            None
        );
        assert!(cycle.accept(PowerSignal::Suspend));
        assert!(cycle.accept(PowerSignal::ResumeAutomatic));
        // Automatic wake started input hours before the user returned. Its
        // worker may have failed while Windows kept the session quiescent.
        let signal = display
            .accept(SESSION_DISPLAY_STATUS, &1u32.to_le_bytes())
            .unwrap();
        assert!(cycle.accept(signal));
        assert!(!cycle.accept(PowerSignal::ResumeInteractive));
    }

    #[test]
    fn malformed_or_unrelated_display_settings_do_not_change_state() {
        let mut display = DisplayPower::default();
        assert_eq!(
            display.accept(SESSION_DISPLAY_STATUS, &0u32.to_le_bytes()),
            None
        );
        assert_eq!(display.accept(GUID::zeroed(), &1u32.to_le_bytes()), None);
        assert_eq!(display.accept(SESSION_DISPLAY_STATUS, &[1]), None);
        assert_eq!(
            display.accept(SESSION_DISPLAY_STATUS, &9u32.to_le_bytes()),
            None
        );
        assert_eq!(
            display.accept(SESSION_DISPLAY_STATUS, &1u32.to_le_bytes()),
            Some(PowerSignal::DisplayOn)
        );
    }

    #[test]
    fn message_window_delivers_display_return_without_installing_input() {
        use windows::Win32::UI::WindowsAndMessaging::SendMessageW;
        #[repr(C)]
        struct Setting {
            guid: GUID,
            length: u32,
            value: u32,
        }
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let monitor = PowerMonitor::spawn(sender).unwrap();
        let window = HWND(monitor.window as *mut c_void);
        for value in [0, 1] {
            let setting = Setting {
                guid: SESSION_DISPLAY_STATUS,
                length: 4,
                value,
            };
            unsafe {
                SendMessageW(
                    window,
                    WM_POWERBROADCAST,
                    Some(WPARAM(PBT_POWERSETTINGCHANGE as usize)),
                    Some(LPARAM(&setting as *const _ as isize)),
                );
            }
        }
        assert_eq!(receiver.try_recv().unwrap(), PowerSignal::DisplayOn);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn maps_suspend_and_all_resume_power_messages() {
        assert_eq!(
            map_power_message(PBT_APMSUSPEND),
            Some(PowerSignal::Suspend)
        );
        assert_eq!(
            map_power_message(PBT_APMRESUMEAUTOMATIC),
            Some(PowerSignal::ResumeAutomatic)
        );
        for message in [
            PBT_APMRESUMESUSPEND,
            PBT_APMRESUMECRITICAL,
            PBT_APMRESUMESTANDBY,
        ] {
            assert_eq!(
                map_power_message(message),
                Some(PowerSignal::ResumeInteractive)
            );
        }
        assert_eq!(map_power_message(0), None);
    }

    #[test]
    fn interactive_resume_does_not_restart_an_automatic_wake_even_much_later() {
        let mut cycle = PowerCycle::default();
        assert!(cycle.accept(PowerSignal::Suspend));
        assert!(cycle.accept(PowerSignal::ResumeAutomatic));
        // No clock is consulted: interactive wake is part of this cycle
        // whether it arrives in milliseconds or hours.
        assert!(!cycle.accept(PowerSignal::ResumeInteractive));
        assert!(!cycle.accept(PowerSignal::ResumeInteractive));
        assert!(cycle.accept(PowerSignal::Suspend));
        assert!(cycle.accept(PowerSignal::ResumeAutomatic));
    }

    #[test]
    fn automatic_resume_recovers_when_suspend_or_interaction_was_missed() {
        let mut cycle = PowerCycle::default();
        assert!(cycle.accept(PowerSignal::ResumeAutomatic));
        // Another unattended sleep/wake with no delivered suspend signal.
        assert!(cycle.accept(PowerSignal::ResumeAutomatic));
        assert!(!cycle.accept(PowerSignal::ResumeInteractive));
    }

    #[test]
    fn interactive_resume_alone_recovers_once_per_observed_sleep() {
        let mut cycle = PowerCycle::default();
        assert!(cycle.accept(PowerSignal::ResumeInteractive));
        assert!(!cycle.accept(PowerSignal::ResumeInteractive));
        cycle.accept(PowerSignal::Suspend);
        assert!(cycle.accept(PowerSignal::ResumeInteractive));
    }
}
