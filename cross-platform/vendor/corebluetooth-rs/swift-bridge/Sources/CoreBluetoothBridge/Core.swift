import CoreBluetooth
import Dispatch
import Foundation

public let CBR_OK: Int32 = 0
public let CBR_INVALID_ARGUMENT: Int32 = -1
public let CBR_FRAMEWORK_ERROR: Int32 = -2
public let CBR_UNKNOWN: Int32 = -99

@inline(__always)
public func cb_retain<T: AnyObject>(_ object: T) -> UnsafeMutableRawPointer {
    Unmanaged.passRetained(object).toOpaque()
}

@inline(__always)
public func cb_borrow<T: AnyObject>(_ ptr: UnsafeMutableRawPointer) -> T {
    Unmanaged<T>.fromOpaque(ptr).takeUnretainedValue()
}

@_cdecl("cb_object_release")
public func cb_object_release(_ ptr: UnsafeMutableRawPointer?) {
    guard let ptr else { return }
    Unmanaged<AnyObject>.fromOpaque(ptr).release()
}

@_cdecl("cb_object_retain")
public func cb_object_retain(_ ptr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    guard let ptr else { return nil }
    _ = Unmanaged<AnyObject>.fromOpaque(ptr).retain()
    return ptr
}

@_cdecl("cb_pointer_array_free")
public func cb_pointer_array_free(_ array: UnsafeMutableRawPointer?, _ count: Int) {
    guard let array else { return }
    let typed = array.assumingMemoryBound(to: UnsafeMutableRawPointer.self)
    typed.deallocate()
    _ = count
}

@inline(__always)
func cb_string(_ value: String) -> UnsafeMutablePointer<CChar>? {
    value.withCString { strdup($0) }
}

@inline(__always)
func cb_write_error(
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?,
    _ message: String
) {
    errorOut?.pointee = cb_string(message)
}

@inline(__always)
func cb_optional(_ value: Any?) -> Any {
    value ?? NSNull()
}

func cb_json_safe(_ value: Any) -> Any {
    switch value {
    case let dict as [String: Any]:
        return dict.mapValues(cb_json_safe)
    case let dict as NSDictionary:
        var object: [String: Any] = [:]
        for (key, value) in dict {
            object[String(describing: key)] = cb_json_safe(value)
        }
        return object
    case let array as [Any]:
        return array.map(cb_json_safe)
    case let array as NSArray:
        return array.map(cb_json_safe)
    case let data as Data:
        return [UInt8](data)
    case let uuid as UUID:
        return uuid.uuidString
    case let uuid as CBUUID:
        return uuid.uuidString
    case let number as NSNumber:
        return number
    case let string as String:
        return string
    case _ as NSNull:
        return NSNull()
    default:
        return String(describing: value)
    }
}

func cb_json_string(_ value: Any) -> String {
    let safe = cb_json_safe(value)

    func encode(_ object: Any) -> String? {
        guard JSONSerialization.isValidJSONObject(object) else {
            return nil
        }
        do {
            let data = try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
            return String(data: data, encoding: .utf8)
        } catch {
            return nil
        }
    }

    if let encoded = encode(safe) {
        return encoded
    }
    if let encodedScalar = encode([safe]) {
        return String(encodedScalar.dropFirst().dropLast())
    }
    return "null"
}

func cb_error_object(_ error: Error) -> [String: Any] {
    let nsError = error as NSError
    return [
        "domain": nsError.domain,
        "code": nsError.code,
        "message": nsError.localizedDescription,
    ]
}

func cb_decode_json<T: Decodable>(_ cString: UnsafePointer<CChar>?, as type: T.Type) throws -> T {
    guard let cString else {
        throw NSError(domain: "corebluetooth-rs", code: Int(CBR_INVALID_ARGUMENT), userInfo: [
            NSLocalizedDescriptionKey: "missing JSON payload",
        ])
    }
    let data = Data(String(cString: cString).utf8)
    return try JSONDecoder().decode(T.self, from: data)
}

func cb_decode_json_if_present<T: Decodable>(_ cString: UnsafePointer<CChar>?, as type: T.Type) throws -> T? {
    guard cString != nil else {
        return nil
    }
    return try cb_decode_json(cString, as: type)
}

func cb_uuid_strings_to_array(_ cString: UnsafePointer<CChar>?) throws -> [String]? {
    try cb_decode_json_if_present(cString, as: [String].self)
}

func cb_service_uuids(_ cString: UnsafePointer<CChar>?) throws -> [CBUUID]? {
    try cb_uuid_strings_to_array(cString)?.map(CBUUID.init(string:))
}

func cb_identifier_uuids(_ cString: UnsafePointer<CChar>?) throws -> [UUID]? {
    guard let strings = try cb_uuid_strings_to_array(cString) else {
        return nil
    }

    return try strings.map { string in
        guard let uuid = UUID(uuidString: string) else {
            throw NSError(domain: "corebluetooth-rs", code: Int(CBR_INVALID_ARGUMENT), userInfo: [
                NSLocalizedDescriptionKey: "invalid UUID string: \(string)",
            ])
        }
        return uuid
    }
}

func cb_retained_handle<T: AnyObject>(_ object: T) -> UInt64 {
    UInt64(UInt(bitPattern: cb_retain(object)))
}

func cb_make_pointer_array<T: AnyObject>(
    _ objects: [T],
    _ outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outCount: UnsafeMutablePointer<Int>
) {
    guard !objects.isEmpty else {
        outArray.pointee = nil
        outCount.pointee = 0
        return
    }

    let buffer = UnsafeMutablePointer<UnsafeMutableRawPointer>.allocate(capacity: objects.count)
    for (index, object) in objects.enumerated() {
        buffer.advanced(by: index).initialize(to: cb_retain(object))
    }
    outArray.pointee = UnsafeMutableRawPointer(buffer)
    outCount.pointee = objects.count
}

func cb_borrow_objects<T: AnyObject>(
    _ array: UnsafePointer<UnsafeMutableRawPointer?>?,
    count: Int,
    cast: (UnsafeMutableRawPointer?) -> T?
) -> [T] {
    guard let array, count > 0 else {
        return []
    }

    return UnsafeBufferPointer(start: array, count: count)
        .compactMap { cast($0) }
}

func cb_peripheral(_ ptr: UnsafeMutableRawPointer?) -> CBPeripheral? {
    guard let ptr else {
        return nil
    }
    let peripheral: CBPeripheral = cb_borrow(ptr)
    return peripheral
}

func cb_service(_ ptr: UnsafeMutableRawPointer?) -> CBService? {
    guard let ptr else {
        return nil
    }
    let service: CBService = cb_borrow(ptr)
    return service
}

func cb_mutable_service(_ ptr: UnsafeMutableRawPointer?) -> CBMutableService? {
    guard let ptr else {
        return nil
    }
    let service: CBMutableService = cb_borrow(ptr)
    return service
}

func cb_characteristic(_ ptr: UnsafeMutableRawPointer?) -> CBCharacteristic? {
    guard let ptr else {
        return nil
    }
    let characteristic: CBCharacteristic = cb_borrow(ptr)
    return characteristic
}

func cb_mutable_characteristic(_ ptr: UnsafeMutableRawPointer?) -> CBMutableCharacteristic? {
    guard let ptr else {
        return nil
    }
    let characteristic: CBMutableCharacteristic = cb_borrow(ptr)
    return characteristic
}

func cb_descriptor(_ ptr: UnsafeMutableRawPointer?) -> CBDescriptor? {
    guard let ptr else {
        return nil
    }
    let descriptor: CBDescriptor = cb_borrow(ptr)
    return descriptor
}

func cb_mutable_descriptor(_ ptr: UnsafeMutableRawPointer?) -> CBMutableDescriptor? {
    guard let ptr else {
        return nil
    }
    let descriptor: CBMutableDescriptor = cb_borrow(ptr)
    return descriptor
}

func cb_uuid(_ ptr: UnsafeMutableRawPointer?) -> CBUUID? {
    guard let ptr else {
        return nil
    }
    let uuid: CBUUID = cb_borrow(ptr)
    return uuid
}

func cb_l2cap_channel(_ ptr: UnsafeMutableRawPointer?) -> CBL2CAPChannel? {
    guard let ptr else {
        return nil
    }
    let channel: CBL2CAPChannel = cb_borrow(ptr)
    return channel
}

func cb_central(_ ptr: UnsafeMutableRawPointer?) -> CBCentral? {
    guard let ptr else {
        return nil
    }
    let central: CBCentral = cb_borrow(ptr)
    return central
}

func cb_att_request(_ ptr: UnsafeMutableRawPointer?) -> CBATTRequest? {
    guard let ptr else {
        return nil
    }
    let request: CBATTRequest = cb_borrow(ptr)
    return request
}

func cb_peer(_ ptr: UnsafeMutableRawPointer?) -> CBPeer? {
    guard let ptr else {
        return nil
    }
    let peer: CBPeer = cb_borrow(ptr)
    return peer
}

func cb_stream(_ ptr: UnsafeMutableRawPointer?) -> Stream? {
    guard let ptr else {
        return nil
    }
    let stream: Stream = cb_borrow(ptr)
    return stream
}

func cb_input_stream(_ ptr: UnsafeMutableRawPointer?) -> InputStream? {
    guard let ptr else {
        return nil
    }
    let stream: InputStream = cb_borrow(ptr)
    return stream
}

func cb_output_stream(_ ptr: UnsafeMutableRawPointer?) -> OutputStream? {
    guard let ptr else {
        return nil
    }
    let stream: OutputStream = cb_borrow(ptr)
    return stream
}
