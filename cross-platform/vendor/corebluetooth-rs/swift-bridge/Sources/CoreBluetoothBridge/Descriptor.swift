import CoreBluetooth
import Foundation

struct CBMutableDescriptorValuePayload: Codable {
    var kind: String
    var string_value: String?
    var bytes_value: [UInt8]?
    var signed_value: Int64?
    var unsigned_value: UInt64?
    var bool_value: Bool?
}

private func cb_descriptor_value(_ payload: CBMutableDescriptorValuePayload) -> Any? {
    switch payload.kind {
    case "string":
        return payload.string_value
    case "bytes":
        return payload.bytes_value.map { Data($0) }
    case "integer":
        return payload.signed_value.map(NSNumber.init(value:))
    case "unsigned":
        return payload.unsigned_value.map(NSNumber.init(value:))
    case "boolean":
        return payload.bool_value.map(NSNumber.init(value:))
    case "null":
        return nil
    default:
        return nil
    }
}

@_cdecl("cb_descriptor_uuid")
public func cb_descriptor_uuid(_ descriptorPtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    cb_descriptor(descriptorPtr).flatMap { cb_string($0.uuid.uuidString) }
}

@_cdecl("cb_descriptor_uuid_handle")
public func cb_descriptor_uuid_handle(_ descriptorPtr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    cb_descriptor(descriptorPtr).map { cb_retain($0.uuid) }
}

@_cdecl("cb_descriptor_characteristic")
public func cb_descriptor_characteristic(_ descriptorPtr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    cb_descriptor(descriptorPtr).flatMap { $0.characteristic.map(cb_retain) }
}

@_cdecl("cb_descriptor_value_json")
public func cb_descriptor_value_json(_ descriptorPtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    guard let descriptor = cb_descriptor(descriptorPtr), let value = descriptor.value else {
        return nil
    }
    return cb_string(cb_json_string(value))
}

@_cdecl("cb_mutable_descriptor_new")
public func cb_mutable_descriptor_new(
    _ uuidPtr: UnsafeMutableRawPointer?,
    _ valueJSON: UnsafePointer<CChar>?,
    _ outDescriptor: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    outDescriptor.pointee = nil
    guard let uuid = cb_uuid(uuidPtr) else {
        cb_write_error(errorOut, "descriptor UUID must not be null")
        return CBR_INVALID_ARGUMENT
    }

    do {
        let payload = try cb_decode_json(valueJSON, as: CBMutableDescriptorValuePayload.self)
        outDescriptor.pointee = cb_retain(CBMutableDescriptor(type: uuid, value: cb_descriptor_value(payload)))
        return CBR_OK
    } catch {
        cb_write_error(errorOut, error.localizedDescription)
        return CBR_INVALID_ARGUMENT
    }
}
