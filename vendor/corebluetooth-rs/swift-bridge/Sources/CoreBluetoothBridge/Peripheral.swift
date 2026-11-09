import CoreBluetooth
import Foundation

public typealias CBPeripheralEventCallback = CBEventCallback

final class CBPeripheralHub: NSObject, CBPeripheralDelegate {
    let streams = CBEventSinkList()
    private let lock = NSLock()
    private var delegateSink: CBEventSink?

    func replaceDelegate(with sink: CBEventSink) {
        lock.lock()
        let previous = delegateSink
        delegateSink = sink
        lock.unlock()
        previous?.deactivate()
    }

    func clearDelegate(context: UnsafeMutableRawPointer?) {
        lock.lock()
        let previous = delegateSink
        if previous?.context == context {
            delegateSink = nil
        }
        lock.unlock()
        if previous?.context == context {
            previous?.deactivate()
        }
    }

    private func broadcast(_ event: String, _ payload: () -> [String: Any]) {
        lock.lock()
        let sink = delegateSink
        lock.unlock()
        sink?.send(event, payload)
        streams.broadcast(event, payload)
    }

    func peripheralDidUpdateName(_ peripheral: CBPeripheral) {
        broadcast("didUpdateName") { [:] }
    }

    func peripheral(_ peripheral: CBPeripheral, didModifyServices invalidatedServices: [CBService]) {
        broadcast("didModifyServices") {
            ["invalidated_service_handles": invalidatedServices.map(cb_retained_handle)]
        }
    }

    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        broadcast("didDiscoverServices") {
            [
                "service_handles": (peripheral.services ?? []).map(cb_retained_handle),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didDiscoverIncludedServicesFor service: CBService,
        error: Error?
    ) {
        broadcast("didDiscoverIncludedServicesForService") {
            [
                "service_handle": cb_retained_handle(service),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didDiscoverCharacteristicsFor service: CBService,
        error: Error?
    ) {
        broadcast("didDiscoverCharacteristicsForService") {
            [
                "service_handle": cb_retained_handle(service),
                "characteristic_handles": (service.characteristics ?? []).map(cb_retained_handle),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didUpdateValueFor characteristic: CBCharacteristic,
        error: Error?
    ) {
        broadcast("didUpdateValueForCharacteristic") {
            [
                "characteristic_handle": cb_retained_handle(characteristic),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didWriteValueFor characteristic: CBCharacteristic,
        error: Error?
    ) {
        broadcast("didWriteValueForCharacteristic") {
            [
                "characteristic_handle": cb_retained_handle(characteristic),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didUpdateNotificationStateFor characteristic: CBCharacteristic,
        error: Error?
    ) {
        broadcast("didUpdateNotificationStateForCharacteristic") {
            [
                "characteristic_handle": cb_retained_handle(characteristic),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didDiscoverDescriptorsFor characteristic: CBCharacteristic,
        error: Error?
    ) {
        broadcast("didDiscoverDescriptorsForCharacteristic") {
            [
                "characteristic_handle": cb_retained_handle(characteristic),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didUpdateValueFor descriptor: CBDescriptor,
        error: Error?
    ) {
        broadcast("didUpdateValueForDescriptor") {
            [
                "descriptor_handle": cb_retained_handle(descriptor),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didWriteValueFor descriptor: CBDescriptor,
        error: Error?
    ) {
        broadcast("didWriteValueForDescriptor") {
            [
                "descriptor_handle": cb_retained_handle(descriptor),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func peripheralIsReady(toSendWriteWithoutResponse peripheral: CBPeripheral) {
        broadcast("isReadyToSendWriteWithoutResponse") { [:] }
    }

    func peripheral(_ peripheral: CBPeripheral, didReadRSSI RSSI: NSNumber, error: Error?) {
        broadcast("didReadRSSI") {
            [
                "rssi": RSSI.intValue,
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didOpen channel: CBL2CAPChannel?,
        error: Error?
    ) {
        broadcast("didOpenL2CAPChannel") {
            [
                "channel_handle": cb_optional(channel.map(cb_retained_handle)),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }
}

private let peripheralHubLock = NSLock()
private var peripheralHubKey: UInt8 = 0

func cb_peripheral_hub(_ peripheral: CBPeripheral) -> CBPeripheralHub {
    peripheralHubLock.lock()
    defer { peripheralHubLock.unlock() }

    if let hub = objc_getAssociatedObject(peripheral, &peripheralHubKey) as? CBPeripheralHub {
        return hub
    }
    let hub = CBPeripheralHub()
    objc_setAssociatedObject(peripheral, &peripheralHubKey, hub, .OBJC_ASSOCIATION_RETAIN)
    peripheral.delegate = hub
    return hub
}

@_cdecl("cb_peripheral_set_delegate")
public func cb_peripheral_set_delegate(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ callback: CBPeripheralEventCallback?,
    _ userInfo: UnsafeMutableRawPointer?,
    _ contextRetain: CBContextCallback?,
    _ contextRelease: CBContextCallback?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr), let callback else {
        cb_write_error(errorOut, "peripheral and callback must not be null")
        return CBR_INVALID_ARGUMENT
    }

    cb_peripheral_hub(peripheral).replaceDelegate(
        with: CBEventSink(
            callback: callback,
            context: userInfo,
            retainContext: contextRetain,
            releaseContext: contextRelease,
            drainsOnDeactivate: false
        )
    )
    return CBR_OK
}

@_cdecl("cb_peripheral_clear_delegate")
public func cb_peripheral_clear_delegate(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ userInfo: UnsafeMutableRawPointer?
) {
    guard let peripheral = cb_peripheral(peripheralPtr) else {
        return
    }
    cb_peripheral_hub(peripheral).clearDelegate(context: userInfo)
}

@_cdecl("cb_peripheral_name")
public func cb_peripheral_name(_ peripheralPtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    cb_peripheral(peripheralPtr).flatMap { cb_string($0.name ?? "") }
}

@_cdecl("cb_peripheral_identifier")
public func cb_peripheral_identifier(_ peripheralPtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    cb_peripheral(peripheralPtr).flatMap { cb_string($0.identifier.uuidString) }
}

@_cdecl("cb_peripheral_state")
public func cb_peripheral_state(_ peripheralPtr: UnsafeMutableRawPointer?) -> Int32 {
    Int32(cb_peripheral(peripheralPtr)?.state.rawValue ?? CBPeripheralState.disconnected.rawValue)
}

@_cdecl("cb_peripheral_services")
public func cb_peripheral_services(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outCount: UnsafeMutablePointer<Int>
) {
    cb_make_pointer_array(cb_peripheral(peripheralPtr)?.services ?? [], outArray, outCount)
}

@_cdecl("cb_peripheral_can_send_write_without_response")
public func cb_peripheral_can_send_write_without_response(_ peripheralPtr: UnsafeMutableRawPointer?) -> Bool {
    cb_peripheral(peripheralPtr)?.canSendWriteWithoutResponse ?? false
}

@_cdecl("cb_peripheral_discover_services")
public func cb_peripheral_discover_services(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ serviceUUIDsJSON: UnsafePointer<CChar>?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr) else {
        cb_write_error(errorOut, "peripheral must not be null")
        return CBR_INVALID_ARGUMENT
    }

    do {
        peripheral.discoverServices(try cb_service_uuids(serviceUUIDsJSON))
        return CBR_OK
    } catch {
        cb_write_error(errorOut, error.localizedDescription)
        return CBR_INVALID_ARGUMENT
    }
}

@_cdecl("cb_peripheral_discover_included_services")
public func cb_peripheral_discover_included_services(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ servicePtr: UnsafeMutableRawPointer?,
    _ serviceUUIDsJSON: UnsafePointer<CChar>?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr), let service = cb_service(servicePtr) else {
        cb_write_error(errorOut, "peripheral and service must not be null")
        return CBR_INVALID_ARGUMENT
    }

    do {
        peripheral.discoverIncludedServices(try cb_service_uuids(serviceUUIDsJSON), for: service)
        return CBR_OK
    } catch {
        cb_write_error(errorOut, error.localizedDescription)
        return CBR_INVALID_ARGUMENT
    }
}

@_cdecl("cb_peripheral_read_rssi")
public func cb_peripheral_read_rssi(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr) else {
        cb_write_error(errorOut, "peripheral must not be null")
        return CBR_INVALID_ARGUMENT
    }

    peripheral.readRSSI()
    return CBR_OK
}

@_cdecl("cb_peripheral_discover_characteristics")
public func cb_peripheral_discover_characteristics(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ servicePtr: UnsafeMutableRawPointer?,
    _ characteristicUUIDsJSON: UnsafePointer<CChar>?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr), let service = cb_service(servicePtr) else {
        cb_write_error(errorOut, "peripheral and service must not be null")
        return CBR_INVALID_ARGUMENT
    }

    do {
        peripheral.discoverCharacteristics(try cb_service_uuids(characteristicUUIDsJSON), for: service)
        return CBR_OK
    } catch {
        cb_write_error(errorOut, error.localizedDescription)
        return CBR_INVALID_ARGUMENT
    }
}

@_cdecl("cb_peripheral_read_value_for_characteristic")
public func cb_peripheral_read_value_for_characteristic(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ characteristicPtr: UnsafeMutableRawPointer?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr), let characteristic = cb_characteristic(characteristicPtr) else {
        cb_write_error(errorOut, "peripheral and characteristic must not be null")
        return CBR_INVALID_ARGUMENT
    }

    peripheral.readValue(for: characteristic)
    return CBR_OK
}

@_cdecl("cb_peripheral_maximum_write_value_length")
public func cb_peripheral_maximum_write_value_length(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ writeType: Int32
) -> Int {
    guard let peripheral = cb_peripheral(peripheralPtr) else {
        return 0
    }
    let type: CBCharacteristicWriteType = writeType == 0 ? .withResponse : .withoutResponse
    return peripheral.maximumWriteValueLength(for: type)
}

@_cdecl("cb_peripheral_write_value_for_characteristic")
public func cb_peripheral_write_value_for_characteristic(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ characteristicPtr: UnsafeMutableRawPointer?,
    _ bytes: UnsafePointer<UInt8>?,
    _ length: Int,
    _ withResponse: Bool,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr), let characteristic = cb_characteristic(characteristicPtr), let bytes else {
        cb_write_error(errorOut, "peripheral, characteristic, and value bytes must not be null")
        return CBR_INVALID_ARGUMENT
    }

    let data = Data(bytes: bytes, count: length)
    peripheral.writeValue(data, for: characteristic, type: withResponse ? .withResponse : .withoutResponse)
    return CBR_OK
}

@_cdecl("cb_peripheral_set_notify_value")
public func cb_peripheral_set_notify_value(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ characteristicPtr: UnsafeMutableRawPointer?,
    _ enabled: Bool,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr), let characteristic = cb_characteristic(characteristicPtr) else {
        cb_write_error(errorOut, "peripheral and characteristic must not be null")
        return CBR_INVALID_ARGUMENT
    }

    peripheral.setNotifyValue(enabled, for: characteristic)
    return CBR_OK
}

@_cdecl("cb_peripheral_discover_descriptors")
public func cb_peripheral_discover_descriptors(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ characteristicPtr: UnsafeMutableRawPointer?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr), let characteristic = cb_characteristic(characteristicPtr) else {
        cb_write_error(errorOut, "peripheral and characteristic must not be null")
        return CBR_INVALID_ARGUMENT
    }

    peripheral.discoverDescriptors(for: characteristic)
    return CBR_OK
}

@_cdecl("cb_peripheral_read_value_for_descriptor")
public func cb_peripheral_read_value_for_descriptor(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ descriptorPtr: UnsafeMutableRawPointer?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr), let descriptor = cb_descriptor(descriptorPtr) else {
        cb_write_error(errorOut, "peripheral and descriptor must not be null")
        return CBR_INVALID_ARGUMENT
    }

    peripheral.readValue(for: descriptor)
    return CBR_OK
}

@_cdecl("cb_peripheral_write_value_for_descriptor")
public func cb_peripheral_write_value_for_descriptor(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ descriptorPtr: UnsafeMutableRawPointer?,
    _ bytes: UnsafePointer<UInt8>?,
    _ length: Int,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr), let descriptor = cb_descriptor(descriptorPtr), let bytes else {
        cb_write_error(errorOut, "peripheral, descriptor, and value bytes must not be null")
        return CBR_INVALID_ARGUMENT
    }
    guard descriptor.uuid != CBUUID(string: CBUUIDClientCharacteristicConfigurationString) else {
        cb_write_error(
            errorOut,
            "the Client Characteristic Configuration descriptor (0x2902) cannot be written; use set_notify_value"
        )
        return CBR_INVALID_ARGUMENT
    }

    peripheral.writeValue(Data(bytes: bytes, count: length), for: descriptor)
    return CBR_OK
}

@_cdecl("cb_peripheral_open_l2cap_channel")
public func cb_peripheral_open_l2cap_channel(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ psm: UInt16,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let peripheral = cb_peripheral(peripheralPtr) else {
        cb_write_error(errorOut, "peripheral must not be null")
        return CBR_INVALID_ARGUMENT
    }

    if #available(macOS 10.14, *) {
        peripheral.openL2CAPChannel(psm)
        return CBR_OK
    }

    cb_write_error(errorOut, "opening L2CAP channels requires macOS 10.14 or newer")
    return CBR_FRAMEWORK_ERROR
}
