//! Read-only UI Automation. All COM objects stay on the worker's MTA thread.
use super::context::{Adapter, RawContext, Status};
#[derive(Clone, Copy)]
enum ApiPath {
    Caret,
    Selection,
}
use windows::{
    core::BOOL,
    Win32::{
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
                COINIT_MULTITHREADED,
            },
            Variant::{VariantClear, VariantToBoolean, VT_BOOL},
        },
        UI::Accessibility::*,
    },
};

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

pub struct WindowsAdapter {
    automation: IUIAutomation,
    _apartment: Apartment,
    last_anchor: Option<IUIAutomationTextRange>,
    position: i64,
}

fn provider<T>(r: windows::core::Result<T>) -> Result<T, Status> {
    r.map_err(|_| Status::ProviderError)
}

impl WindowsAdapter {
    pub fn new() -> Result<Self, Status> {
        // The process entry thread has not initialized COM previously.
        unsafe {
            provider(CoInitializeEx(None, COINIT_MULTITHREADED).ok())?;
            let apartment = Apartment;
            let automation =
                provider(CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER))?;
            Ok(Self {
                automation,
                _apartment: apartment,
                last_anchor: None,
                position: 0,
            })
        }
    }

    unsafe fn text_provider(
        &self,
        focused: &IUIAutomationElement,
    ) -> Result<(IUIAutomationElement, IUIAutomationTextPattern), Status> {
        unsafe {
            if let Ok(pattern) =
                focused.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
            {
                return Ok((focused.clone(), pattern));
            }
            // Only follow the provider's explicit text-container relationship. A broad
            // ancestor search risks capturing an unrelated document or whole window.
            let child = focused
                .GetCurrentPatternAs::<IUIAutomationTextChildPattern>(UIA_TextChildPatternId)
                .map_err(|_| Status::Unsupported)?;
            let container = provider(child.TextContainer())?;
            if provider(container.CurrentIsPassword())?.as_bool() {
                return Err(Status::Protected);
            }
            let pattern = container
                .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
                .map_err(|_| Status::Unsupported)?;
            Ok((container, pattern))
        }
    }

    unsafe fn anchor(
        &self,
        element: &IUIAutomationElement,
        pattern: &IUIAutomationTextPattern,
    ) -> Result<(IUIAutomationTextRange, bool, ApiPath), Status> {
        unsafe {
            // A selection takes precedence over the caret, whose active end could be
            // either end of the selected text. Reject multiple selections outright.
            let ranges = provider(pattern.GetSelection())?;
            let count = provider(ranges.Length())?;
            if count > 1 {
                return Err(Status::AmbiguousSelection);
            }
            if count < 0 {
                return Err(Status::ProviderError);
            }
            let selection = if count == 1 {
                Some(provider(ranges.GetElement(0))?)
            } else {
                None
            };
            if let Some(range) = &selection {
                if provider(range.CompareEndpoints(
                    TextPatternRangeEndpoint_Start,
                    range,
                    TextPatternRangeEndpoint_End,
                ))? != 0
                {
                    return Ok((provider(range.Clone())?, true, ApiPath::Selection));
                }
            }
            if let Ok(pattern2) =
                element.GetCurrentPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id)
            {
                let mut active = BOOL(0);
                if let Ok(caret) = pattern2.GetCaretRange(&mut active) {
                    if active.as_bool() {
                        if provider(caret.CompareEndpoints(
                            TextPatternRangeEndpoint_Start,
                            &caret,
                            TextPatternRangeEndpoint_End,
                        ))? != 0
                        {
                            return Err(Status::ProviderError);
                        }
                        return Ok((caret, false, ApiPath::Caret));
                    }
                }
            }
            selection
                .map(|s| (s, false, ApiPath::Selection))
                .ok_or(Status::Unsupported)
        }
    }
}

impl Adapter for WindowsAdapter {
    type Target = IUIAutomationElement;
    fn focused(&mut self) -> Result<Self::Target, Status> {
        unsafe {
            self.automation
                .GetFocusedElement()
                .map_err(|_| Status::NoFocus)
        }
    }
    fn protected(&mut self, target: &Self::Target) -> Result<bool, Status> {
        unsafe { Ok(provider(target.CurrentIsPassword())?.as_bool()) }
    }
    fn editable(&mut self, target: &Self::Target) -> bool {
        unsafe {
            target
                .CurrentControlType()
                .is_ok_and(|c| c == UIA_EditControlTypeId)
                && target.CurrentIsEnabled().is_ok_and(|v| v.as_bool())
                && target
                    .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                    .and_then(|p| p.CurrentIsReadOnly())
                    .is_ok_and(|v| !v.as_bool())
        }
    }
    fn same(&mut self, a: &Self::Target, b: &Self::Target) -> Result<bool, Status> {
        unsafe { Ok(provider(self.automation.CompareElements(a, b))?.as_bool()) }
    }
    fn read(&mut self, target: &Self::Target) -> Result<RawContext, Status> {
        unsafe {
            let (element, pattern) = self.text_provider(target)?;
            if provider(element.CurrentIsPassword())?.as_bool() {
                return Err(Status::Protected);
            }
            let (anchor, has_selection, _path) = self.anchor(&element, &pattern)?;
            let mut value = provider(anchor.GetAttributeValue(UIA_IsReadOnlyAttributeId))?;
            if value.Anonymous.Anonymous.vt != VT_BOOL {
                let _ = VariantClear(&mut value);
                return Err(Status::Unsupported);
            }
            let readonly = VariantToBoolean(&value);
            let _ = VariantClear(&mut value);
            if readonly.map_err(|_| Status::Unsupported)?.as_bool() {
                return Err(Status::Unsupported);
            }
            // Range manipulation is local to the UIA range object; no Select() or
            // mutation API is ever used, and nothing is copied to the clipboard.
            let range = provider(anchor.Clone())?;
            provider(range.MoveEndpointByRange(
                TextPatternRangeEndpoint_End,
                &anchor,
                TextPatternRangeEndpoint_Start,
            ))?;
            let moved = provider(range.MoveEndpointByUnit(
                TextPatternRangeEndpoint_Start,
                TextUnit_Character,
                -512,
            ))?;
            if !(-512..=0).contains(&moved) {
                return Err(Status::ProviderError);
            }
            let document = provider(pattern.DocumentRange())?;
            let clipped_start = provider(range.CompareEndpoints(
                TextPatternRangeEndpoint_Start,
                &document,
                TextPatternRangeEndpoint_Start,
            ))? > 0;
            // Providers differ in their definition of a Character unit. Read a
            // bounded buffer and reject possible truncation rather than return
            // text from the wrong end of the range.
            let text = provider(range.GetText(2048))?;
            if text.len() >= 2048 {
                return Err(Status::Unsupported);
            }
            let text = String::from_utf16(&text).map_err(|_| Status::ProviderError)?;
            let chars: Vec<char> = text.chars().collect();
            let skip = chars.len().saturating_sub(512);
            if self.last_anchor.as_ref().is_none_or(|old| {
                anchor.CompareEndpoints(
                    TextPatternRangeEndpoint_Start,
                    old,
                    TextPatternRangeEndpoint_Start,
                ) != Ok(0)
            }) {
                self.position = self.position.wrapping_add(1);
            }
            self.last_anchor = Some(provider(anchor.Clone())?);
            let right = provider(anchor.Clone())?;
            provider(right.MoveEndpointByRange(
                TextPatternRangeEndpoint_End,
                &anchor,
                TextPatternRangeEndpoint_Start,
            ))?;
            provider(right.MoveEndpointByUnit(
                TextPatternRangeEndpoint_End,
                TextUnit_Character,
                2,
            ))?;
            let after = provider(right.GetText(16))?;
            if after.len() >= 16 {
                return Err(Status::Unsupported);
            }
            Ok(RawContext {
                before: chars[skip..].iter().collect(),
                clipped_start: clipped_start || skip > 0,
                has_selection,
                after: String::from_utf16(&after).map_err(|_| Status::ProviderError)?,
                position: self.position,
            })
        }
    }
}
