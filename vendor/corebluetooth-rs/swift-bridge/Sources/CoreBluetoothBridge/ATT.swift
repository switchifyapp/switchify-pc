import CoreBluetooth
import Foundation

@_cdecl("cb_att_request_central")
public func cb_att_request_central(_ requestPtr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    cb_att_request(requestPtr).map { cb_retain($0.central) }
}

@_cdecl("cb_att_request_characteristic")
public func cb_att_request_characteristic(_ requestPtr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    cb_att_request(requestPtr).map { cb_retain($0.characteristic) }
}

@_cdecl("cb_att_request_offset")
public func cb_att_request_offset(_ requestPtr: UnsafeMutableRawPointer?) -> Int {
    cb_att_request(requestPtr)?.offset ?? 0
}

@_cdecl("cb_att_request_value_json")
public func cb_att_request_value_json(_ requestPtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    guard let request = cb_att_request(requestPtr), let value = request.value else {
        return nil
    }
    return cb_string(cb_json_string([UInt8](value)))
}

@_cdecl("cb_att_request_set_value")
public func cb_att_request_set_value(
    _ requestPtr: UnsafeMutableRawPointer?,
    _ bytes: UnsafePointer<UInt8>?,
    _ length: Int
) {
    cb_att_request(requestPtr)?.value = bytes.map { Data(bytes: $0, count: length) }
}

@_cdecl("cb_error_domain")
public func cb_error_domain() -> UnsafeMutablePointer<CChar>? {
    cb_string(CBErrorDomain)
}

@_cdecl("cb_att_error_domain")
public func cb_att_error_domain() -> UnsafeMutablePointer<CChar>? {
    cb_string(CBATTErrorDomain)
}
