import CoreBluetooth
import Foundation

private func cb_uuid_constant_value(_ kind: Int32) -> String {
    switch kind {
    case 0:
        return CBUUIDCharacteristicExtendedPropertiesString
    case 1:
        return CBUUIDCharacteristicUserDescriptionString
    case 2:
        return CBUUIDClientCharacteristicConfigurationString
    case 3:
        return CBUUIDServerCharacteristicConfigurationString
    case 4:
        return CBUUIDCharacteristicFormatString
    case 5:
        return CBUUIDCharacteristicAggregateFormatString
    case 6:
        return CBUUIDCharacteristicValidRangeString
    case 7:
        return CBUUIDCharacteristicObservationScheduleString
    case 8:
        return CBUUIDL2CAPPSMCharacteristicString
    default:
        return CBUUIDCharacteristicUserDescriptionString
    }
}

@_cdecl("cb_uuid_new_from_string")
public func cb_uuid_new_from_string(_ uuid: UnsafePointer<CChar>?) -> UnsafeMutableRawPointer? {
    guard let uuid else { return nil }
    return cb_retain(CBUUID(string: String(cString: uuid)))
}

@_cdecl("cb_uuid_new_from_bytes")
public func cb_uuid_new_from_bytes(
    _ bytes: UnsafePointer<UInt8>?,
    _ length: Int
) -> UnsafeMutableRawPointer? {
    let data = bytes.map { Data(bytes: $0, count: length) } ?? Data()
    return cb_retain(CBUUID(data: data))
}

@_cdecl("cb_uuid_string")
public func cb_uuid_string(_ uuidPtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    cb_uuid(uuidPtr).flatMap { cb_string($0.uuidString) }
}

@_cdecl("cb_uuid_data_json")
public func cb_uuid_data_json(_ uuidPtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    guard let uuid = cb_uuid(uuidPtr) else { return nil }
    return cb_string(cb_json_string([UInt8](uuid.data)))
}

@_cdecl("cb_uuid_constant_string")
public func cb_uuid_constant_string(_ kind: Int32) -> UnsafeMutablePointer<CChar>? {
    cb_string(cb_uuid_constant_value(kind))
}
