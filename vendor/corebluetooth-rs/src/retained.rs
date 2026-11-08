//! Declarative macro for retain/release wrapper boilerplate.
//!
//! Many wrapper types hold a single `*mut c_void` pointer to a retained
//! `CoreBluetooth` / Swift object and hand-roll identical `Clone` (retain) and
//! `Drop` (release) implementations. `cb_retained!` consolidates that
//! boilerplate into a single audited place.
//!
//! The generated impls preserve the exact behavior of the previous
//! hand-written versions:
//! - `Clone` bumps the retain count by calling `retain_raw` and rebuilds the
//!   wrapper through its `from_retained_raw` constructor.
//! - `Drop` releases the pointer via `cb_object_release` (which null-checks
//!   internally), matching the original hand-written versions.
//!
//! Types whose `Clone`/`Drop` carry extra logic beyond retain/release (e.g.
//! `Peripheral`, `CentralManager`, and `PeripheralManager` delegate refcount
//! bookkeeping) are intentionally left hand-written.

/// Generate `Clone` and `Drop` impls for a retain/release pointer wrapper.
///
/// The wrapper must expose a `from_retained_raw(*mut c_void) -> Self`
/// associated function and store its retained pointer in a `raw` field.
///
/// Usage: `cb_retained!(Ty);`
macro_rules! cb_retained {
    ($ty:ty) => {
        impl Clone for $ty {
            fn clone(&self) -> Self {
                Self::from_retained_raw($crate::private::retain_raw(self.raw))
            }
        }

        impl Drop for $ty {
            fn drop(&mut self) {
                unsafe { $crate::ffi::cb_object_release(self.raw) };
            }
        }
    };
}

pub(crate) use cb_retained;
