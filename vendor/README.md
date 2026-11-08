# Vendored dependencies

`corebluetooth-rs` 0.4.2 is vendored under its original MIT/Apache-2.0 license. It differs from the published crate in two places:

- Its Swift UUID bridge references `CBUUIDCharacteristicObservationScheduleString`, a constant available to the SDK but not exported by current macOS CoreBluetooth at runtime. The vendored copy substitutes the Bluetooth SIG-assigned descriptor UUID literal `2906`.
- `MutableService::is_same_service` and `MutableCharacteristic::is_same_characteristic` compare the underlying CoreBluetooth objects, so the macOS peripheral can match add-service callbacks and ATT requests to its own service and characteristic.
