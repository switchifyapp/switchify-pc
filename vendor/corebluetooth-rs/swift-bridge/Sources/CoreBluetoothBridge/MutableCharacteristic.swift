import CoreBluetooth
import Foundation

@_cdecl("cb_mutable_characteristic_new")
public func cb_mutable_characteristic_new(
    _ uuidPtr: UnsafeMutableRawPointer?,
    _ properties: UInt64,
    _ bytes: UnsafePointer<UInt8>?,
    _ length: Int,
    _ permissions: UInt64,
    _ outCharacteristic: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    outCharacteristic.pointee = nil
    guard let uuid = cb_uuid(uuidPtr) else {
        cb_write_error(errorOut, "characteristic UUID must not be null")
        return CBR_INVALID_ARGUMENT
    }

    let value = bytes.map { Data(bytes: $0, count: length) }
    let characteristic = CBMutableCharacteristic(
        type: uuid,
        properties: CBCharacteristicProperties(rawValue: UInt(properties)),
        value: value,
        permissions: CBAttributePermissions(rawValue: UInt(permissions))
    )
    outCharacteristic.pointee = cb_retain(characteristic)
    return CBR_OK
}

@_cdecl("cb_mutable_characteristic_permissions")
public func cb_mutable_characteristic_permissions(_ characteristicPtr: UnsafeMutableRawPointer?) -> UInt64 {
    UInt64(cb_mutable_characteristic(characteristicPtr)?.permissions.rawValue ?? 0)
}

@_cdecl("cb_mutable_characteristic_set_permissions")
public func cb_mutable_characteristic_set_permissions(
    _ characteristicPtr: UnsafeMutableRawPointer?,
    _ permissions: UInt64
) {
    cb_mutable_characteristic(characteristicPtr)?.permissions = CBAttributePermissions(rawValue: UInt(permissions))
}

@_cdecl("cb_mutable_characteristic_set_properties")
public func cb_mutable_characteristic_set_properties(
    _ characteristicPtr: UnsafeMutableRawPointer?,
    _ properties: UInt64
) {
    cb_mutable_characteristic(characteristicPtr)?.properties = CBCharacteristicProperties(rawValue: UInt(properties))
}

@_cdecl("cb_mutable_characteristic_set_value")
public func cb_mutable_characteristic_set_value(
    _ characteristicPtr: UnsafeMutableRawPointer?,
    _ bytes: UnsafePointer<UInt8>?,
    _ length: Int
) {
    cb_mutable_characteristic(characteristicPtr)?.value = bytes.map { Data(bytes: $0, count: length) }
}

@_cdecl("cb_mutable_characteristic_subscribed_centrals")
public func cb_mutable_characteristic_subscribed_centrals(
    _ characteristicPtr: UnsafeMutableRawPointer?,
    _ outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outCount: UnsafeMutablePointer<Int>
) {
    cb_make_pointer_array(cb_mutable_characteristic(characteristicPtr)?.subscribedCentrals ?? [], outArray, outCount)
}

@_cdecl("cb_mutable_characteristic_set_descriptors")
public func cb_mutable_characteristic_set_descriptors(
    _ characteristicPtr: UnsafeMutableRawPointer?,
    _ descriptors: UnsafePointer<UnsafeMutableRawPointer?>?,
    _ count: Int,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let characteristic = cb_mutable_characteristic(characteristicPtr) else {
        cb_write_error(errorOut, "mutable characteristic must not be null")
        return CBR_INVALID_ARGUMENT
    }

    characteristic.descriptors = cb_borrow_objects(descriptors, count: count, cast: cb_descriptor)
    return CBR_OK
}
