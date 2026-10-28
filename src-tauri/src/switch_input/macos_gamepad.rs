//! Controller buttons on macOS, read as HID reports through IOHIDManager on
//! the capture thread's run loop. The GameController framework needs a flag
//! for background delivery that stopped working in macOS 15.4; HID reports
//! arrive whichever app is in front and need no window.
use super::{gamepad::Gamepads, gamepad_name, Driver, GAMEPAD_BUTTONS};
use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{kCFAllocatorDefault, CFAllocatorRef, CFRelease, CFType, TCFType},
    dictionary::{CFDictionary, CFDictionaryRef},
    number::CFNumber,
    runloop::{kCFRunLoopDefaultMode, CFRunLoop, CFRunLoopRef},
    string::{CFString, CFStringRef},
};
use std::{cell::RefCell, ffi::c_void};

type IOHIDManagerRef = *mut c_void;
type IOHIDValueRef = *mut c_void;
type IOHIDElementRef = *mut c_void;
type IOHIDDeviceRef = *mut c_void;
type IOReturn = i32;
type ValueCallback = unsafe extern "C" fn(*mut c_void, IOReturn, *mut c_void, IOHIDValueRef);
type DeviceCallback = unsafe extern "C" fn(*mut c_void, IOReturn, *mut c_void, IOHIDDeviceRef);

const USAGE_PAGE_GENERIC_DESKTOP: i32 = 1;
const USAGE_JOYSTICK: i32 = 4;
const USAGE_GAMEPAD: i32 = 5;
const USAGE_PAGE_BUTTON: u32 = 9;
const IO_RETURN_SUCCESS: IOReturn = 0;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: u32) -> IOHIDManagerRef;
    fn IOHIDManagerSetDeviceMatchingMultiple(manager: IOHIDManagerRef, multiple: CFArrayRef);
    fn IOHIDManagerSetInputValueMatching(manager: IOHIDManagerRef, matching: CFDictionaryRef);
    fn IOHIDManagerRegisterInputValueCallback(
        manager: IOHIDManagerRef,
        callback: Option<ValueCallback>,
        context: *mut c_void,
    );
    fn IOHIDManagerRegisterDeviceRemovalCallback(
        manager: IOHIDManagerRef,
        callback: Option<DeviceCallback>,
        context: *mut c_void,
    );
    fn IOHIDManagerScheduleWithRunLoop(
        manager: IOHIDManagerRef,
        run_loop: CFRunLoopRef,
        mode: CFStringRef,
    );
    fn IOHIDManagerUnscheduleFromRunLoop(
        manager: IOHIDManagerRef,
        run_loop: CFRunLoopRef,
        mode: CFStringRef,
    );
    fn IOHIDManagerOpen(manager: IOHIDManagerRef, options: u32) -> IOReturn;
    fn IOHIDManagerClose(manager: IOHIDManagerRef, options: u32) -> IOReturn;
    fn IOHIDValueGetElement(value: IOHIDValueRef) -> IOHIDElementRef;
    fn IOHIDValueGetIntegerValue(value: IOHIDValueRef) -> isize;
    fn IOHIDElementGetUsagePage(element: IOHIDElementRef) -> u32;
    fn IOHIDElementGetUsage(element: IOHIDElementRef) -> u32;
    fn IOHIDElementGetDevice(element: IOHIDElementRef) -> IOHIDDeviceRef;
}

struct Context {
    driver: Driver,
    pads: RefCell<Gamepads>,
}
impl Context {
    fn deliver(&self, edges: Vec<(u8, bool)>) {
        for (button, pressed) in edges {
            self.driver.key(&gamepad_name(button), pressed);
        }
    }
}

unsafe extern "C" fn value_changed(
    context: *mut c_void,
    _result: IOReturn,
    _sender: *mut c_void,
    value: IOHIDValueRef,
) {
    let context = &*(context as *const Context);
    let element = IOHIDValueGetElement(value);
    if element.is_null() || IOHIDElementGetUsagePage(element) != USAGE_PAGE_BUTTON {
        return;
    }
    let usage = IOHIDElementGetUsage(element);
    if !(1..=u32::from(GAMEPAD_BUTTONS)).contains(&usage) {
        return;
    }
    let device = IOHIDElementGetDevice(element) as isize;
    let pressed = IOHIDValueGetIntegerValue(value) != 0;
    let edges = context
        .pads
        .borrow_mut()
        .button(device, usage as u8, pressed);
    context.deliver(edges);
}

unsafe extern "C" fn device_removed(
    context: *mut c_void,
    _result: IOReturn,
    _sender: *mut c_void,
    device: IOHIDDeviceRef,
) {
    let context = &*(context as *const Context);
    let edges = context.pads.borrow_mut().removed(device as isize);
    context.deliver(edges);
}

fn usage_matching(usage: i32) -> CFType {
    CFDictionary::from_CFType_pairs(&[
        (
            CFString::from_static_string("DeviceUsagePage"),
            CFNumber::from(USAGE_PAGE_GENERIC_DESKTOP),
        ),
        (
            CFString::from_static_string("DeviceUsage"),
            CFNumber::from(usage),
        ),
    ])
    .as_CFType()
}

/// Controller monitoring scheduled on the calling thread's run loop. Drop it
/// on that thread; the callbacks are detached before the state is freed.
pub(super) struct Monitor {
    manager: IOHIDManagerRef,
    context: *mut Context,
    run_loop: CFRunLoop,
}
impl Monitor {
    /// `None` when HID access is unavailable; keyboard capture carries on.
    pub fn start(driver: Driver) -> Option<Self> {
        unsafe {
            let manager = IOHIDManagerCreate(kCFAllocatorDefault, 0);
            if manager.is_null() {
                return None;
            }
            let devices = CFArray::from_CFTypes(&[
                usage_matching(USAGE_JOYSTICK),
                usage_matching(USAGE_GAMEPAD),
            ]);
            IOHIDManagerSetDeviceMatchingMultiple(manager, devices.as_concrete_TypeRef());
            // Only button elements call back; sticks and triggers never reach us.
            let buttons = CFDictionary::from_CFType_pairs(&[(
                CFString::from_static_string("UsagePage"),
                CFNumber::from(USAGE_PAGE_BUTTON as i32),
            )]);
            IOHIDManagerSetInputValueMatching(manager, buttons.as_concrete_TypeRef());
            let context = Box::into_raw(Box::new(Context {
                driver,
                pads: RefCell::new(Gamepads::default()),
            }));
            IOHIDManagerRegisterInputValueCallback(manager, Some(value_changed), context.cast());
            IOHIDManagerRegisterDeviceRemovalCallback(
                manager,
                Some(device_removed),
                context.cast(),
            );
            let run_loop = CFRunLoop::get_current();
            IOHIDManagerScheduleWithRunLoop(
                manager,
                run_loop.as_concrete_TypeRef(),
                kCFRunLoopDefaultMode,
            );
            let monitor = Self {
                manager,
                context,
                run_loop,
            };
            if IOHIDManagerOpen(manager, 0) != IO_RETURN_SUCCESS {
                // Drop unschedules, closes and frees in the right order.
                drop(monitor);
                return None;
            }
            Some(monitor)
        }
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        unsafe {
            IOHIDManagerUnscheduleFromRunLoop(
                self.manager,
                self.run_loop.as_concrete_TypeRef(),
                kCFRunLoopDefaultMode,
            );
            IOHIDManagerRegisterInputValueCallback(self.manager, None, std::ptr::null_mut());
            IOHIDManagerRegisterDeviceRemovalCallback(self.manager, None, std::ptr::null_mut());
            IOHIDManagerClose(self.manager, 0);
            CFRelease(self.manager);
            drop(Box::from_raw(self.context));
        }
    }
}
