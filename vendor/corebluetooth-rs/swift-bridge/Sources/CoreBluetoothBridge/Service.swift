import CoreBluetooth
import Foundation

@_cdecl("cb_service_uuid")
public func cb_service_uuid(_ servicePtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    cb_service(servicePtr).flatMap { cb_string($0.uuid.uuidString) }
}

@_cdecl("cb_service_uuid_handle")
public func cb_service_uuid_handle(_ servicePtr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    cb_service(servicePtr).map { cb_retain($0.uuid) }
}

@_cdecl("cb_service_peripheral")
public func cb_service_peripheral(_ servicePtr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    cb_service(servicePtr).flatMap { $0.peripheral.map(cb_retain) }
}

@_cdecl("cb_service_is_primary")
public func cb_service_is_primary(_ servicePtr: UnsafeMutableRawPointer?) -> Bool {
    cb_service(servicePtr)?.isPrimary ?? false
}

@_cdecl("cb_service_included_services")
public func cb_service_included_services(
    _ servicePtr: UnsafeMutableRawPointer?,
    _ outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outCount: UnsafeMutablePointer<Int>
) {
    cb_make_pointer_array(cb_service(servicePtr)?.includedServices ?? [], outArray, outCount)
}

@_cdecl("cb_service_characteristics")
public func cb_service_characteristics(
    _ servicePtr: UnsafeMutableRawPointer?,
    _ outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outCount: UnsafeMutablePointer<Int>
) {
    cb_make_pointer_array(cb_service(servicePtr)?.characteristics ?? [], outArray, outCount)
}
