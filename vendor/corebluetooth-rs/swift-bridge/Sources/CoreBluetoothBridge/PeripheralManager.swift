import CoreBluetooth
import Dispatch
import Foundation

struct CBPeripheralManagerOptionsPayload: Codable {
    var queue_label: String?
    var show_power_alert: Bool?
    var restore_identifier: String?
}

public typealias CBPeripheralManagerEventCallback =
    @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void

private func cb_peripheral_manager_authorization_value(_ manager: CBPeripheralManager?) -> Int32 {
    guard manager != nil else {
        return 0
    }

    if #available(macOS 10.15, *) {
        return Int32(CBManager.authorization.rawValue)
    }

    return 0
}

private func cb_peripheral_manager_options(_ payload: CBPeripheralManagerOptionsPayload) -> [String: Any]? {
    var options: [String: Any] = [:]
    if let showPowerAlert = payload.show_power_alert {
        options[CBPeripheralManagerOptionShowPowerAlertKey] = NSNumber(value: showPowerAlert)
    }
    if let restoreIdentifier = payload.restore_identifier {
        options[CBPeripheralManagerOptionRestoreIdentifierKey] = restoreIdentifier
    }
    return options.isEmpty ? nil : options
}

private final class CBRustPeripheralManagerDelegate: NSObject, CBPeripheralManagerDelegate {
    let callback: CBPeripheralManagerEventCallback
    let userInfo: UnsafeMutableRawPointer?
    private var isActive = true

    init(callback: @escaping CBPeripheralManagerEventCallback, userInfo: UnsafeMutableRawPointer?) {
        self.callback = callback
        self.userInfo = userInfo
        super.init()
    }

    func deactivate() {
        isActive = false
    }

    private func send(_ payload: [String: Any]) {
        guard isActive else { return }
        let json = cb_json_string(payload)
        json.withCString { callback(userInfo, $0) }
    }

    func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        send([
            "event": "didUpdateState",
            "state": peripheral.state.rawValue,
            "authorization": cb_peripheral_manager_authorization_value(peripheral),
        ])
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, willRestoreState dict: [String: Any]) {
        send([
            "event": "willRestoreState",
            "service_handles": (dict[CBPeripheralManagerRestoredStateServicesKey] as? [CBMutableService] ?? []).map(cb_retained_handle),
            "advertisement_data": cb_optional(dict[CBPeripheralManagerRestoredStateAdvertisementDataKey]),
        ])
    }

    func peripheralManagerDidStartAdvertising(_ peripheral: CBPeripheralManager, error: Error?) {
        send([
            "event": "didStartAdvertising",
            "error": cb_optional(error.map(cb_error_object)),
        ])
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didAdd service: CBService, error: Error?) {
        send([
            "event": "didAddService",
            "service_handle": cb_retained_handle(service),
            "error": cb_optional(error.map(cb_error_object)),
        ])
    }

    func peripheralManager(
        _ peripheral: CBPeripheralManager,
        central: CBCentral,
        didSubscribeTo characteristic: CBCharacteristic
    ) {
        send([
            "event": "didSubscribeToCharacteristic",
            "central_handle": cb_retained_handle(central),
            "characteristic_handle": cb_retained_handle(characteristic),
        ])
    }

    func peripheralManager(
        _ peripheral: CBPeripheralManager,
        central: CBCentral,
        didUnsubscribeFrom characteristic: CBCharacteristic
    ) {
        send([
            "event": "didUnsubscribeFromCharacteristic",
            "central_handle": cb_retained_handle(central),
            "characteristic_handle": cb_retained_handle(characteristic),
        ])
    }

    func peripheralManagerIsReady(toUpdateSubscribers peripheral: CBPeripheralManager) {
        send(["event": "isReadyToUpdateSubscribers"])
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveRead request: CBATTRequest) {
        send([
            "event": "didReceiveReadRequest",
            "request_handle": cb_retained_handle(request),
        ])
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveWrite requests: [CBATTRequest]) {
        send([
            "event": "didReceiveWriteRequests",
            "request_handles": requests.map(cb_retained_handle),
        ])
    }

    func peripheralManager(
        _ peripheral: CBPeripheralManager,
        didPublishL2CAPChannel psm: CBL2CAPPSM,
        error: Error?
    ) {
        send([
            "event": "didPublishL2CAPChannel",
            "psm": Int(psm),
            "error": cb_optional(error.map(cb_error_object)),
        ])
    }

    func peripheralManager(
        _ peripheral: CBPeripheralManager,
        didUnpublishL2CAPChannel psm: CBL2CAPPSM,
        error: Error?
    ) {
        send([
            "event": "didUnpublishL2CAPChannel",
            "psm": Int(psm),
            "error": cb_optional(error.map(cb_error_object)),
        ])
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
    }
}

private final class CBPeripheralManagerBox: NSObject {
    let manager: CBPeripheralManager
    let delegateBox: CBRustPeripheralManagerDelegate?
    let queue: DispatchQueue

    init(
        manager: CBPeripheralManager,
        delegateBox: CBRustPeripheralManagerDelegate?,
        queue: DispatchQueue
    ) {
        self.manager = manager
        self.delegateBox = delegateBox
        self.queue = queue
        super.init()
        self.manager.delegate = delegateBox
    }

    deinit {
        delegateBox?.deactivate()
        manager.delegate = nil
    }
}

private func cb_peripheral_manager_box(_ ptr: UnsafeMutableRawPointer?) -> CBPeripheralManagerBox? {
    guard let ptr else {
        return nil
    }
    let box: CBPeripheralManagerBox = cb_borrow(ptr)
    return box
}

@_cdecl("cb_peripheral_manager_new")
public func cb_peripheral_manager_new(
    _ optionsJSON: UnsafePointer<CChar>?,
    _ callback: CBPeripheralManagerEventCallback?,
    _ userInfo: UnsafeMutableRawPointer?,
    _ outManager: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    outManager.pointee = nil

    do {
        let payload = try cb_decode_json_if_present(optionsJSON, as: CBPeripheralManagerOptionsPayload.self) ?? CBPeripheralManagerOptionsPayload()
        let queue = DispatchQueue(label: payload.queue_label ?? "corebluetooth-rs.peripheral")
        let delegateBox = callback.map { CBRustPeripheralManagerDelegate(callback: $0, userInfo: userInfo) }
        let manager = CBPeripheralManager(delegate: nil, queue: queue, options: cb_peripheral_manager_options(payload))
        let box = CBPeripheralManagerBox(manager: manager, delegateBox: delegateBox, queue: queue)
        outManager.pointee = cb_retain(box)
        return CBR_OK
    } catch {
        cb_write_error(errorOut, error.localizedDescription)
        return CBR_INVALID_ARGUMENT
    }
}

@_cdecl("cb_peripheral_manager_state")
public func cb_peripheral_manager_state(_ managerPtr: UnsafeMutableRawPointer?) -> Int32 {
    Int32(cb_peripheral_manager_box(managerPtr)?.manager.state.rawValue ?? CBManagerState.unknown.rawValue)
}

@_cdecl("cb_peripheral_manager_authorization")
public func cb_peripheral_manager_authorization(_ managerPtr: UnsafeMutableRawPointer?) -> Int32 {
    cb_peripheral_manager_authorization_value(cb_peripheral_manager_box(managerPtr)?.manager)
}

@_cdecl("cb_peripheral_manager_global_authorization")
public func cb_peripheral_manager_global_authorization() -> Int32 {
    if #available(macOS 10.15, *) {
        return Int32(CBManager.authorization.rawValue)
    }
    return 0
}

@_cdecl("cb_peripheral_manager_is_advertising")
public func cb_peripheral_manager_is_advertising(_ managerPtr: UnsafeMutableRawPointer?) -> Bool {
    cb_peripheral_manager_box(managerPtr)?.manager.isAdvertising ?? false
}

@_cdecl("cb_peripheral_manager_start_advertising")
public func cb_peripheral_manager_start_advertising(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ advertisementJSON: UnsafePointer<CChar>?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let manager = cb_peripheral_manager_box(managerPtr)?.manager else {
        cb_write_error(errorOut, "peripheral manager must not be null")
        return CBR_INVALID_ARGUMENT
    }

    do {
        manager.startAdvertising(try cb_advertisement_dictionary(advertisementJSON))
        return CBR_OK
    } catch {
        cb_write_error(errorOut, error.localizedDescription)
        return CBR_INVALID_ARGUMENT
    }
}

@_cdecl("cb_peripheral_manager_stop_advertising")
public func cb_peripheral_manager_stop_advertising(_ managerPtr: UnsafeMutableRawPointer?) {
    cb_peripheral_manager_box(managerPtr)?.manager.stopAdvertising()
}

@_cdecl("cb_peripheral_manager_set_desired_connection_latency")
public func cb_peripheral_manager_set_desired_connection_latency(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ latency: Int32,
    _ centralPtr: UnsafeMutableRawPointer?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let manager = cb_peripheral_manager_box(managerPtr)?.manager, let central = cb_central(centralPtr) else {
        cb_write_error(errorOut, "peripheral manager and central must not be null")
        return CBR_INVALID_ARGUMENT
    }

    let targetLatency: CBPeripheralManagerConnectionLatency
    switch latency {
    case 1:
        targetLatency = .medium
    case 2:
        targetLatency = .high
    default:
        targetLatency = .low
    }
    manager.setDesiredConnectionLatency(targetLatency, for: central)
    return CBR_OK
}

@_cdecl("cb_peripheral_manager_add_service")
public func cb_peripheral_manager_add_service(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ servicePtr: UnsafeMutableRawPointer?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let manager = cb_peripheral_manager_box(managerPtr)?.manager, let service = cb_mutable_service(servicePtr) else {
        cb_write_error(errorOut, "peripheral manager and service must not be null")
        return CBR_INVALID_ARGUMENT
    }

    manager.add(service)
    return CBR_OK
}

@_cdecl("cb_peripheral_manager_remove_service")
public func cb_peripheral_manager_remove_service(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ servicePtr: UnsafeMutableRawPointer?
) {
    guard let manager = cb_peripheral_manager_box(managerPtr)?.manager, let service = cb_mutable_service(servicePtr) else {
        return
    }

    manager.remove(service)
}

@_cdecl("cb_peripheral_manager_remove_all_services")
public func cb_peripheral_manager_remove_all_services(_ managerPtr: UnsafeMutableRawPointer?) {
    cb_peripheral_manager_box(managerPtr)?.manager.removeAllServices()
}

@_cdecl("cb_peripheral_manager_respond_to_request")
public func cb_peripheral_manager_respond_to_request(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ requestPtr: UnsafeMutableRawPointer?,
    _ result: Int32,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let manager = cb_peripheral_manager_box(managerPtr)?.manager, let request = cb_att_request(requestPtr) else {
        cb_write_error(errorOut, "peripheral manager and request must not be null")
        return CBR_INVALID_ARGUMENT
    }

    let attResult = CBATTError.Code(rawValue: Int(result)) ?? .success
    manager.respond(to: request, withResult: attResult)
    return CBR_OK
}

@_cdecl("cb_peripheral_manager_update_value")
public func cb_peripheral_manager_update_value(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ bytes: UnsafePointer<UInt8>?,
    _ length: Int,
    _ characteristicPtr: UnsafeMutableRawPointer?,
    _ centrals: UnsafePointer<UnsafeMutableRawPointer?>?,
    _ count: Int,
    _ outSent: UnsafeMutablePointer<Bool>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let manager = cb_peripheral_manager_box(managerPtr)?.manager, let characteristic = cb_mutable_characteristic(characteristicPtr) else {
        cb_write_error(errorOut, "peripheral manager and characteristic must not be null")
        return CBR_INVALID_ARGUMENT
    }

    let value = bytes.map { Data(bytes: $0, count: length) } ?? Data()
    let targetCentrals = cb_borrow_objects(centrals, count: count, cast: cb_central)
    outSent.pointee = manager.updateValue(value, for: characteristic, onSubscribedCentrals: targetCentrals.isEmpty ? nil : targetCentrals)
    return CBR_OK
}

@_cdecl("cb_peripheral_manager_publish_l2cap_channel")
public func cb_peripheral_manager_publish_l2cap_channel(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ encryptionRequired: Bool,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let manager = cb_peripheral_manager_box(managerPtr)?.manager else {
        cb_write_error(errorOut, "peripheral manager must not be null")
        return CBR_INVALID_ARGUMENT
    }

    if #available(macOS 10.14, *) {
        manager.publishL2CAPChannel(withEncryption: encryptionRequired)
        return CBR_OK
    }

    cb_write_error(errorOut, "publishing L2CAP channels requires macOS 10.14 or newer")
    return CBR_FRAMEWORK_ERROR
}

@_cdecl("cb_peripheral_manager_unpublish_l2cap_channel")
public func cb_peripheral_manager_unpublish_l2cap_channel(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ psm: UInt16,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let manager = cb_peripheral_manager_box(managerPtr)?.manager else {
        cb_write_error(errorOut, "peripheral manager must not be null")
        return CBR_INVALID_ARGUMENT
    }

    if #available(macOS 10.14, *) {
        manager.unpublishL2CAPChannel(psm)
        return CBR_OK
    }

    cb_write_error(errorOut, "unpublishing L2CAP channels requires macOS 10.14 or newer")
    return CBR_FRAMEWORK_ERROR
}

@_cdecl("cb_central_identifier")
public func cb_central_identifier(_ centralPtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    cb_central(centralPtr).flatMap { cb_string($0.identifier.uuidString) }
}

@_cdecl("cb_central_maximum_update_value_length")
public func cb_central_maximum_update_value_length(_ centralPtr: UnsafeMutableRawPointer?) -> Int {
    cb_central(centralPtr)?.maximumUpdateValueLength ?? 0
}

func cb_peripheral_manager_get_manager(_ ptr: UnsafeMutableRawPointer?) -> CBPeripheralManager? {
    cb_peripheral_manager_box(ptr)?.manager
}
