import CoreBluetooth
import Foundation

@_cdecl("cb_mutable_service_new")
public func cb_mutable_service_new(
    _ uuidPtr: UnsafeMutableRawPointer?,
    _ isPrimary: Bool,
    _ outService: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    outService.pointee = nil
    guard let uuid = cb_uuid(uuidPtr) else {
        cb_write_error(errorOut, "service UUID must not be null")
        return CBR_INVALID_ARGUMENT
    }

    outService.pointee = cb_retain(CBMutableService(type: uuid, primary: isPrimary))
    return CBR_OK
}

@_cdecl("cb_mutable_service_set_included_services")
public func cb_mutable_service_set_included_services(
    _ servicePtr: UnsafeMutableRawPointer?,
    _ services: UnsafePointer<UnsafeMutableRawPointer?>?,
    _ count: Int,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let service = cb_mutable_service(servicePtr) else {
        cb_write_error(errorOut, "mutable service must not be null")
        return CBR_INVALID_ARGUMENT
    }

    service.includedServices = cb_borrow_objects(services, count: count, cast: cb_service)
    return CBR_OK
}

@_cdecl("cb_mutable_service_set_characteristics")
public func cb_mutable_service_set_characteristics(
    _ servicePtr: UnsafeMutableRawPointer?,
    _ characteristics: UnsafePointer<UnsafeMutableRawPointer?>?,
    _ count: Int,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let service = cb_mutable_service(servicePtr) else {
        cb_write_error(errorOut, "mutable service must not be null")
        return CBR_INVALID_ARGUMENT
    }

    service.characteristics = cb_borrow_objects(characteristics, count: count, cast: cb_characteristic)
    return CBR_OK
}
