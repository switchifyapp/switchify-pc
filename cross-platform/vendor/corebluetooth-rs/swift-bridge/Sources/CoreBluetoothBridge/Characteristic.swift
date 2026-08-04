import CoreBluetooth
import Foundation

@_cdecl("cb_characteristic_uuid")
public func cb_characteristic_uuid(_ characteristicPtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    cb_characteristic(characteristicPtr).flatMap { cb_string($0.uuid.uuidString) }
}

@_cdecl("cb_characteristic_uuid_handle")
public func cb_characteristic_uuid_handle(_ characteristicPtr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    cb_characteristic(characteristicPtr).map { cb_retain($0.uuid) }
}

@_cdecl("cb_characteristic_service")
public func cb_characteristic_service(_ characteristicPtr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    cb_characteristic(characteristicPtr).flatMap { $0.service.map(cb_retain) }
}

@_cdecl("cb_characteristic_properties")
public func cb_characteristic_properties(_ characteristicPtr: UnsafeMutableRawPointer?) -> UInt64 {
    UInt64(cb_characteristic(characteristicPtr)?.properties.rawValue ?? 0)
}

@_cdecl("cb_characteristic_value_json")
public func cb_characteristic_value_json(_ characteristicPtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    guard let characteristic = cb_characteristic(characteristicPtr), let value = characteristic.value else {
        return nil
    }
    return cb_string(cb_json_string([UInt8](value)))
}

@_cdecl("cb_characteristic_is_notifying")
public func cb_characteristic_is_notifying(_ characteristicPtr: UnsafeMutableRawPointer?) -> Bool {
    cb_characteristic(characteristicPtr)?.isNotifying ?? false
}

@_cdecl("cb_characteristic_descriptors")
public func cb_characteristic_descriptors(
    _ characteristicPtr: UnsafeMutableRawPointer?,
    _ outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outCount: UnsafeMutablePointer<Int>
) {
    cb_make_pointer_array(cb_characteristic(characteristicPtr)?.descriptors ?? [], outArray, outCount)
}

