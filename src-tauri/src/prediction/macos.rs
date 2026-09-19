use super::context::{Adapter, RawContext, Status};
use core_foundation::{
    base::{CFEqual, CFRange, CFType, CFTypeRef, TCFType},
    string::{CFString, CFStringRef},
};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(e: CFTypeRef, a: CFStringRef, out: *mut CFTypeRef) -> i32;
    fn AXUIElementCopyParameterizedAttributeValue(
        e: CFTypeRef,
        a: CFStringRef,
        p: CFTypeRef,
        out: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementIsAttributeSettable(e: CFTypeRef, a: CFStringRef, out: *mut u8) -> i32;
    fn AXUIElementSetMessagingTimeout(e: CFTypeRef, seconds: f32) -> i32;
    fn AXValueCreate(kind: u32, value: *const std::ffi::c_void) -> CFTypeRef;
    fn AXValueGetValue(value: CFTypeRef, kind: u32, result: *mut std::ffi::c_void) -> bool;
}
#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn IsSecureEventInputEnabled() -> bool;
}
fn attribute(e: &CFType, key: &str) -> Result<CFType, Status> {
    let mut out = std::ptr::null();
    let key = CFString::new(key);
    if unsafe {
        AXUIElementCopyAttributeValue(e.as_CFTypeRef(), key.as_concrete_TypeRef(), &mut out)
    } != 0
        || out.is_null()
    {
        return Err(Status::Unsupported);
    }
    Ok(unsafe { CFType::wrap_under_create_rule(out) })
}
fn string(value: CFType) -> Result<String, Status> {
    if value.type_of() != CFString::type_id() {
        return Err(Status::Unsupported);
    }
    Ok(unsafe { CFString::wrap_under_get_rule(value.as_CFTypeRef().cast()) }.to_string())
}
pub struct MacAdapter {
    system: CFType,
}
impl MacAdapter {
    pub fn new() -> Result<Self, Status> {
        let system = unsafe { AXUIElementCreateSystemWide() };
        if system.is_null() {
            return Err(Status::NoFocus);
        }
        unsafe {
            AXUIElementSetMessagingTimeout(system, 0.2);
        }
        Ok(Self {
            system: unsafe { CFType::wrap_under_create_rule(system) },
        })
    }
    fn text(&self, target: &CFType, range: CFRange) -> Result<String, Status> {
        let parameter = unsafe { AXValueCreate(4, (&range as *const CFRange).cast()) };
        if parameter.is_null() {
            return Err(Status::ProviderError);
        }
        let parameter = unsafe { CFType::wrap_under_create_rule(parameter) };
        let key = CFString::new("AXStringForRange");
        let mut out = std::ptr::null();
        if unsafe {
            AXUIElementCopyParameterizedAttributeValue(
                target.as_CFTypeRef(),
                key.as_concrete_TypeRef(),
                parameter.as_CFTypeRef(),
                &mut out,
            )
        } != 0
            || out.is_null()
        {
            return Err(Status::Unsupported);
        }
        string(unsafe { CFType::wrap_under_create_rule(out) })
    }
}
impl Adapter for MacAdapter {
    type Target = CFType;
    fn focused(&mut self) -> Result<CFType, Status> {
        attribute(&self.system, "AXFocusedUIElement")
    }
    fn protected(&mut self, target: &CFType) -> Result<bool, Status> {
        if unsafe { IsSecureEventInputEnabled() } {
            return Ok(true);
        }
        let role = string(attribute(target, "AXRole")?)?;
        let subrole = attribute(target, "AXSubrole")
            .and_then(string)
            .unwrap_or_default();
        Ok(role == "AXSecureTextField"
            || subrole == "AXSecureTextField"
            || !matches!(role.as_str(), "AXTextField" | "AXTextArea" | "AXComboBox"))
    }
    fn editable(&mut self, target: &CFType) -> bool {
        let mut settable = 0;
        let key = CFString::new("AXValue");
        unsafe {
            AXUIElementIsAttributeSettable(
                target.as_CFTypeRef(),
                key.as_concrete_TypeRef(),
                &mut settable,
            ) == 0
                && settable != 0
        }
    }
    fn same(&mut self, a: &CFType, b: &CFType) -> Result<bool, Status> {
        Ok(unsafe { CFEqual(a.as_CFTypeRef(), b.as_CFTypeRef()) } != 0)
    }
    fn read(&mut self, target: &CFType) -> Result<RawContext, Status> {
        if !self.editable(target) {
            return Err(Status::Unsupported);
        }
        if let Ok(ranges) = attribute(target, "AXSelectedTextRanges") {
            use core_foundation::array::CFArray;
            if ranges.type_of() != CFArray::<CFType>::type_id() {
                return Err(Status::Unsupported);
            }
            let array =
                unsafe { CFArray::<CFType>::wrap_under_get_rule(ranges.as_CFTypeRef().cast()) };
            if array.len() != 1 {
                return Err(Status::AmbiguousSelection);
            }
        }
        let selected = attribute(target, "AXSelectedTextRange")?;
        let mut range = CFRange {
            location: 0,
            length: 0,
        };
        if !unsafe {
            AXValueGetValue(
                selected.as_CFTypeRef(),
                4,
                (&mut range as *mut CFRange).cast(),
            )
        } || range.location < 0
            || range.length < 0
        {
            return Err(Status::Unsupported);
        }
        let start = range.location.saturating_sub(512).max(0);
        let before = self.text(
            target,
            CFRange {
                location: start,
                length: range.location - start,
            },
        )?;
        // AX providers may reject a range past the document end. Obtain its
        // character count without reading the whole value.
        use core_foundation::number::CFNumber;
        let count = attribute(target, "AXNumberOfCharacters")?;
        if count.type_of() != CFNumber::type_id() {
            return Err(Status::Unsupported);
        }
        let count = unsafe { CFNumber::wrap_under_get_rule(count.as_CFTypeRef().cast()) }
            .to_i64()
            .ok_or(Status::Unsupported)?;
        let remaining = count.saturating_sub(range.location as i64);
        if remaining < 0 {
            return Err(Status::Unsupported);
        }
        let after = self.text(
            target,
            CFRange {
                location: range.location,
                length: (remaining as isize).min(2),
            },
        )?;
        if before.chars().count() > 512 || after.chars().count() > 2 {
            return Err(Status::Unsupported);
        }
        Ok(RawContext {
            before,
            after,
            clipped_start: start > 0,
            has_selection: range.length != 0,
            position: range.location as i64,
        })
    }
}
