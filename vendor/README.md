# Vendored dependencies

`corebluetooth-rs` 0.3.6 is vendored under its original MIT/Apache-2.0 license. Its Swift UUID bridge references `CBUUIDCharacteristicObservationScheduleString`, a constant available to the SDK but not exported by current macOS CoreBluetooth at runtime. The vendored copy substitutes the Bluetooth SIG-assigned descriptor UUID literal `2906` and is otherwise unchanged from the published crate.
