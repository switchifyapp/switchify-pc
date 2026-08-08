import CoreBluetooth
import Foundation

// MARK: - Authorization helper

private func cb_async_authorization_value() -> Int32 {
    if #available(macOS 10.15, *) {
        return Int32(CBManager.authorization.rawValue)
    }
    return 0
}

// MARK: - CentralManager stream bridge

final class CBCentralManagerStreamBridge: NSObject, CBCentralManagerDelegate {
    let onEvent: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void
    let ctx: UnsafeMutableRawPointer?
    let previousDelegate: CBCentralManagerDelegate?

    init(
        onEvent: @escaping @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void,
        ctx: UnsafeMutableRawPointer?,
        wrapping previous: CBCentralManagerDelegate?
    ) {
        self.onEvent = onEvent
        self.ctx = ctx
        self.previousDelegate = previous
        super.init()
    }

    private func send(_ payload: [String: Any]) {
        let json = cb_json_string(payload)
        json.withCString { onEvent(ctx, $0) }
    }

    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        send([
            "event": "didUpdateState",
            "state": central.state.rawValue,
            "authorization": cb_async_authorization_value(),
        ])
        previousDelegate?.centralManagerDidUpdateState(central)
    }

    func centralManager(_ central: CBCentralManager, willRestoreState dict: [String: Any]) {
        previousDelegate?.centralManager?(central, willRestoreState: dict)
    }

    func centralManager(
        _ central: CBCentralManager,
        didDiscover peripheral: CBPeripheral,
        advertisementData: [String: Any],
        rssi RSSI: NSNumber
    ) {
        send([
            "event": "didDiscoverPeripheral",
            "peripheral_handle": cb_retained_handle(peripheral),
            "rssi": RSSI.intValue,
            "advertisement_data": advertisementData,
        ])
        previousDelegate?.centralManager?(central, didDiscover: peripheral, advertisementData: advertisementData, rssi: RSSI)
    }

    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        send([
            "event": "didConnectPeripheral",
            "peripheral_handle": cb_retained_handle(peripheral),
        ])
        previousDelegate?.centralManager?(central, didConnect: peripheral)
    }

    func centralManager(
        _ central: CBCentralManager,
        didFailToConnect peripheral: CBPeripheral,
        error: Error?
    ) {
        send([
            "event": "didFailToConnectPeripheral",
            "peripheral_handle": cb_retained_handle(peripheral),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.centralManager?(central, didFailToConnect: peripheral, error: error)
    }

    func centralManager(
        _ central: CBCentralManager,
        didDisconnectPeripheral peripheral: CBPeripheral,
        error: Error?
    ) {
        send([
            "event": "didDisconnectPeripheral",
            "peripheral_handle": cb_retained_handle(peripheral),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.centralManager?(central, didDisconnectPeripheral: peripheral, error: error)
    }

    func centralManager(
        _ central: CBCentralManager,
        didDisconnectPeripheral peripheral: CBPeripheral,
        timestamp: CFAbsoluteTime,
        isReconnecting: Bool,
        error: Error?
    ) {
        previousDelegate?.centralManager?(
            central,
            didDisconnectPeripheral: peripheral,
            timestamp: timestamp,
            isReconnecting: isReconnecting,
            error: error
        )
    }
}

@_cdecl("cb_central_manager_stream_subscribe")
public func cb_central_manager_stream_subscribe(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ onEvent: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void,
    _ ctx: UnsafeMutableRawPointer?
) -> UnsafeMutableRawPointer? {
    guard let manager = cb_central_manager_get_manager(managerPtr) else { return nil }
    let bridge = CBCentralManagerStreamBridge(
        onEvent: onEvent,
        ctx: ctx,
        wrapping: manager.delegate
    )
    manager.delegate = bridge
    return Unmanaged.passRetained(bridge).toOpaque()
}

@_cdecl("cb_central_manager_stream_unsubscribe")
public func cb_central_manager_stream_unsubscribe(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ bridgePtr: UnsafeMutableRawPointer?
) {
    guard let bridgePtr else { return }
    let bridge = Unmanaged<CBCentralManagerStreamBridge>.fromOpaque(bridgePtr).takeRetainedValue()
    if let manager = cb_central_manager_get_manager(managerPtr) {
        if manager.delegate === bridge {
            if let prev = bridge.previousDelegate as? NSObject & CBCentralManagerDelegate {
                manager.delegate = prev
            } else {
                manager.delegate = nil
            }
        }
    }
}

// MARK: - Peripheral stream bridge

final class CBPeripheralStreamBridge: NSObject, CBPeripheralDelegate {
    let onEvent: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void
    let ctx: UnsafeMutableRawPointer?
    let previousDelegate: CBPeripheralDelegate?

    init(
        onEvent: @escaping @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void,
        ctx: UnsafeMutableRawPointer?,
        wrapping previous: CBPeripheralDelegate?
    ) {
        self.onEvent = onEvent
        self.ctx = ctx
        self.previousDelegate = previous
        super.init()
    }

    private func send(_ payload: [String: Any]) {
        let json = cb_json_string(payload)
        json.withCString { onEvent(ctx, $0) }
    }

    func peripheralDidUpdateName(_ peripheral: CBPeripheral) {
        send(["event": "didUpdateName"])
        previousDelegate?.peripheralDidUpdateName?(peripheral)
    }

    func peripheral(_ peripheral: CBPeripheral, didModifyServices invalidatedServices: [CBService]) {
        send([
            "event": "didModifyServices",
            "invalidated_service_handles": invalidatedServices.map(cb_retained_handle),
        ])
        previousDelegate?.peripheral?(peripheral, didModifyServices: invalidatedServices)
    }

    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        send([
            "event": "didDiscoverServices",
            "service_handles": (peripheral.services ?? []).map(cb_retained_handle),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheral?(peripheral, didDiscoverServices: error)
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didDiscoverIncludedServicesFor service: CBService,
        error: Error?
    ) {
        send([
            "event": "didDiscoverIncludedServicesForService",
            "service_handle": cb_retained_handle(service),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheral?(peripheral, didDiscoverIncludedServicesFor: service, error: error)
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didDiscoverCharacteristicsFor service: CBService,
        error: Error?
    ) {
        send([
            "event": "didDiscoverCharacteristicsForService",
            "service_handle": cb_retained_handle(service),
            "characteristic_handles": (service.characteristics ?? []).map(cb_retained_handle),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheral?(peripheral, didDiscoverCharacteristicsFor: service, error: error)
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didUpdateValueFor characteristic: CBCharacteristic,
        error: Error?
    ) {
        send([
            "event": "didUpdateValueForCharacteristic",
            "characteristic_handle": cb_retained_handle(characteristic),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheral?(peripheral, didUpdateValueFor: characteristic, error: error)
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didWriteValueFor characteristic: CBCharacteristic,
        error: Error?
    ) {
        send([
            "event": "didWriteValueForCharacteristic",
            "characteristic_handle": cb_retained_handle(characteristic),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheral?(peripheral, didWriteValueFor: characteristic, error: error)
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didUpdateNotificationStateFor characteristic: CBCharacteristic,
        error: Error?
    ) {
        send([
            "event": "didUpdateNotificationStateForCharacteristic",
            "characteristic_handle": cb_retained_handle(characteristic),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheral?(peripheral, didUpdateNotificationStateFor: characteristic, error: error)
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didDiscoverDescriptorsFor characteristic: CBCharacteristic,
        error: Error?
    ) {
        send([
            "event": "didDiscoverDescriptorsForCharacteristic",
            "characteristic_handle": cb_retained_handle(characteristic),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheral?(peripheral, didDiscoverDescriptorsFor: characteristic, error: error)
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didUpdateValueFor descriptor: CBDescriptor,
        error: Error?
    ) {
        send([
            "event": "didUpdateValueForDescriptor",
            "descriptor_handle": cb_retained_handle(descriptor),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheral?(peripheral, didUpdateValueFor: descriptor, error: error)
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didWriteValueFor descriptor: CBDescriptor,
        error: Error?
    ) {
        send([
            "event": "didWriteValueForDescriptor",
            "descriptor_handle": cb_retained_handle(descriptor),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheral?(peripheral, didWriteValueFor: descriptor, error: error)
    }

    func peripheralIsReady(toSendWriteWithoutResponse peripheral: CBPeripheral) {
        send(["event": "isReadyToSendWriteWithoutResponse"])
        previousDelegate?.peripheralIsReady?(toSendWriteWithoutResponse: peripheral)
    }

    func peripheral(_ peripheral: CBPeripheral, didReadRSSI RSSI: NSNumber, error: Error?) {
        send([
            "event": "didReadRSSI",
            "rssi": RSSI.intValue,
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheral?(peripheral, didReadRSSI: RSSI, error: error)
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didOpen channel: CBL2CAPChannel?,
        error: Error?
    ) {
        send([
            "event": "didOpenL2CAPChannel",
            "channel_handle": cb_optional(channel.map(cb_retained_handle)),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheral?(peripheral, didOpen: channel, error: error)
    }
}

@_cdecl("cb_peripheral_stream_subscribe")
public func cb_peripheral_stream_subscribe(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ onEvent: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void,
    _ ctx: UnsafeMutableRawPointer?
) -> UnsafeMutableRawPointer? {
    guard let peripheral = cb_peripheral(peripheralPtr) else { return nil }
    let bridge = CBPeripheralStreamBridge(
        onEvent: onEvent,
        ctx: ctx,
        wrapping: peripheral.delegate
    )
    peripheral.delegate = bridge
    return Unmanaged.passRetained(bridge).toOpaque()
}

@_cdecl("cb_peripheral_stream_unsubscribe")
public func cb_peripheral_stream_unsubscribe(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ bridgePtr: UnsafeMutableRawPointer?
) {
    guard let bridgePtr else { return }
    let bridge = Unmanaged<CBPeripheralStreamBridge>.fromOpaque(bridgePtr).takeRetainedValue()
    if let peripheral = cb_peripheral(peripheralPtr) {
        if peripheral.delegate === bridge {
            if let prev = bridge.previousDelegate as? NSObject & CBPeripheralDelegate {
                peripheral.delegate = prev
            } else {
                peripheral.delegate = nil
            }
        }
    }
}

// MARK: - PeripheralManager stream bridge

final class CBPeripheralManagerStreamBridge: NSObject, CBPeripheralManagerDelegate {
    let onEvent: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void
    let ctx: UnsafeMutableRawPointer?
    let previousDelegate: CBPeripheralManagerDelegate?

    init(
        onEvent: @escaping @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void,
        ctx: UnsafeMutableRawPointer?,
        wrapping previous: CBPeripheralManagerDelegate?
    ) {
        self.onEvent = onEvent
        self.ctx = ctx
        self.previousDelegate = previous
        super.init()
    }

    private func send(_ payload: [String: Any]) {
        let json = cb_json_string(payload)
        json.withCString { onEvent(ctx, $0) }
    }

    func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        send([
            "event": "didUpdateState",
            "state": peripheral.state.rawValue,
            "authorization": cb_async_authorization_value(),
        ])
        previousDelegate?.peripheralManagerDidUpdateState(peripheral)
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, willRestoreState dict: [String: Any]) {
        previousDelegate?.peripheralManager?(peripheral, willRestoreState: dict)
    }

    func peripheralManagerDidStartAdvertising(_ peripheral: CBPeripheralManager, error: Error?) {
        send([
            "event": "didStartAdvertising",
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheralManagerDidStartAdvertising?(peripheral, error: error)
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didAdd service: CBService, error: Error?) {
        send([
            "event": "didAddService",
            "service_handle": cb_retained_handle(service),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheralManager?(peripheral, didAdd: service, error: error)
    }

    func peripheralManager(
        _ peripheral: CBPeripheralManager,
        central: CBCentral,
        didSubscribeTo characteristic: CBCharacteristic
    ) {
        send([
            "event": "didSubscribeCentral",
            "central_handle": cb_retained_handle(central),
            "characteristic_handle": cb_retained_handle(characteristic),
        ])
        previousDelegate?.peripheralManager?(peripheral, central: central, didSubscribeTo: characteristic)
    }

    func peripheralManager(
        _ peripheral: CBPeripheralManager,
        central: CBCentral,
        didUnsubscribeFrom characteristic: CBCharacteristic
    ) {
        send([
            "event": "didUnsubscribeCentral",
            "central_handle": cb_retained_handle(central),
            "characteristic_handle": cb_retained_handle(characteristic),
        ])
        previousDelegate?.peripheralManager?(peripheral, central: central, didUnsubscribeFrom: characteristic)
    }

    func peripheralManagerIsReady(toUpdateSubscribers peripheral: CBPeripheralManager) {
        send(["event": "isReadyToUpdateSubscribers"])
        previousDelegate?.peripheralManagerIsReady?(toUpdateSubscribers: peripheral)
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveRead request: CBATTRequest) {
        send([
            "event": "didReceiveReadRequest",
            "request_handle": cb_retained_handle(request),
        ])
        previousDelegate?.peripheralManager?(peripheral, didReceiveRead: request)
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveWrite requests: [CBATTRequest]) {
        send([
            "event": "didReceiveWriteRequests",
            "request_handles": requests.map(cb_retained_handle),
        ])
        previousDelegate?.peripheralManager?(peripheral, didReceiveWrite: requests)
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didPublishL2CAPChannel PSM: CBL2CAPPSM, error: Error?) {
        send([
            "event": "didPublishL2CAPChannel",
            "psm": Int(PSM),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheralManager?(peripheral, didPublishL2CAPChannel: PSM, error: error)
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didUnpublishL2CAPChannel PSM: CBL2CAPPSM, error: Error?) {
        send([
            "event": "didUnpublishL2CAPChannel",
            "psm": Int(PSM),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheralManager?(peripheral, didUnpublishL2CAPChannel: PSM, error: error)
    }

    func peripheralManager(
        _ peripheral: CBPeripheralManager,
        didOpen channel: CBL2CAPChannel?,
        error: Error?
    ) {
        send([
            "event": "didOpenL2CAPChannel",
            "channel_handle": cb_optional(channel.map(cb_retained_handle)),
            "error": cb_optional(error.map(cb_error_object)),
        ])
        previousDelegate?.peripheralManager?(peripheral, didOpen: channel, error: error)
    }
}

@_cdecl("cb_peripheral_manager_stream_subscribe")
public func cb_peripheral_manager_stream_subscribe(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ onEvent: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void,
    _ ctx: UnsafeMutableRawPointer?
) -> UnsafeMutableRawPointer? {
    guard let manager = cb_peripheral_manager_get_manager(managerPtr) else { return nil }
    let bridge = CBPeripheralManagerStreamBridge(
        onEvent: onEvent,
        ctx: ctx,
        wrapping: manager.delegate
    )
    manager.delegate = bridge
    return Unmanaged.passRetained(bridge).toOpaque()
}

@_cdecl("cb_peripheral_manager_stream_unsubscribe")
public func cb_peripheral_manager_stream_unsubscribe(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ bridgePtr: UnsafeMutableRawPointer?
) {
    guard let bridgePtr else { return }
    let bridge = Unmanaged<CBPeripheralManagerStreamBridge>.fromOpaque(bridgePtr).takeRetainedValue()
    if let manager = cb_peripheral_manager_get_manager(managerPtr) {
        if manager.delegate === bridge {
            if let prev = bridge.previousDelegate as? NSObject & CBPeripheralManagerDelegate {
                manager.delegate = prev
            } else {
                manager.delegate = nil
            }
        }
    }
}
