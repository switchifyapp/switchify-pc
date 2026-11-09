import CoreBluetooth
import Dispatch
import Foundation

struct CBCentralManagerOptionsPayload: Codable {
    var queue_label: String?
    var show_power_alert: Bool?
    var restore_identifier: String?
}

struct CBCentralManagerScanOptionsPayload: Codable {
    var allow_duplicates: Bool
    var solicited_service_uuids: [String] = []
}

struct CBCentralManagerConnectOptionsPayload: Codable {
    var notify_on_connection: Bool?
    var notify_on_disconnection: Bool?
    var notify_on_notification: Bool?
    var start_delay_seconds: Double?
    var enable_auto_reconnect: Bool?
}

public typealias CBCentralManagerEventCallback = CBEventCallback

private func cb_manager_authorization_value(_ manager: CBCentralManager?) -> Int32 {
    guard manager != nil else {
        return 0
    }

    if #available(macOS 10.15, *) {
        return Int32(CBManager.authorization.rawValue)
    }

    return 0
}

private func cb_central_manager_options(_ payload: CBCentralManagerOptionsPayload) -> [String: Any]? {
    var options: [String: Any] = [:]
    if let showPowerAlert = payload.show_power_alert {
        options[CBCentralManagerOptionShowPowerAlertKey] = NSNumber(value: showPowerAlert)
    }
    if let restoreIdentifier = payload.restore_identifier {
        options[CBCentralManagerOptionRestoreIdentifierKey] = restoreIdentifier
    }
    return options.isEmpty ? nil : options
}

private func cb_scan_options(_ payload: CBCentralManagerScanOptionsPayload) -> [String: Any]? {
    var options: [String: Any] = [:]
    if payload.allow_duplicates {
        options[CBCentralManagerScanOptionAllowDuplicatesKey] = NSNumber(value: true)
    }
    if !payload.solicited_service_uuids.isEmpty {
        options[CBCentralManagerScanOptionSolicitedServiceUUIDsKey] = payload.solicited_service_uuids.map(CBUUID.init(string:))
    }
    return options.isEmpty ? nil : options
}

private func cb_restored_scan_options_payload(_ options: [String: Any]?) -> [String: Any]? {
    guard let options else {
        return nil
    }

    return [
        "allow_duplicates": (options[CBCentralManagerScanOptionAllowDuplicatesKey] as? NSNumber)?.boolValue ?? false,
        "solicited_service_uuids": (options[CBCentralManagerScanOptionSolicitedServiceUUIDsKey] as? [CBUUID] ?? []).map(\.uuidString),
    ]
}

private func cb_connect_options(_ payload: CBCentralManagerConnectOptionsPayload) -> [String: Any]? {
    var options: [String: Any] = [:]
    if let notifyOnConnection = payload.notify_on_connection {
        options[CBConnectPeripheralOptionNotifyOnConnectionKey] = NSNumber(value: notifyOnConnection)
    }
    if let notifyOnDisconnection = payload.notify_on_disconnection {
        options[CBConnectPeripheralOptionNotifyOnDisconnectionKey] = NSNumber(value: notifyOnDisconnection)
    }
    if let notifyOnNotification = payload.notify_on_notification {
        options[CBConnectPeripheralOptionNotifyOnNotificationKey] = NSNumber(value: notifyOnNotification)
    }
    if let startDelaySeconds = payload.start_delay_seconds {
        options[CBConnectPeripheralOptionStartDelayKey] = NSNumber(value: startDelaySeconds)
    }
    if #available(macOS 14.0, *), let enableAutoReconnect = payload.enable_auto_reconnect {
        options[CBConnectPeripheralOptionEnableAutoReconnect] = NSNumber(value: enableAutoReconnect)
    }
    return options.isEmpty ? nil : options
}

final class CBCentralManagerHub: NSObject, CBCentralManagerDelegate {
    let sinks = CBEventSinkList()

    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        sinks.broadcast("didUpdateState") {
            [
                "state": central.state.rawValue,
                "authorization": cb_manager_authorization_value(central),
            ]
        }
    }

    func centralManager(_ central: CBCentralManager, willRestoreState dict: [String: Any]) {
        sinks.broadcast("willRestoreState") {
            [
                "peripheral_handles": (dict[CBCentralManagerRestoredStatePeripheralsKey] as? [CBPeripheral] ?? []).map(cb_retained_handle),
                "scan_service_uuids": (dict[CBCentralManagerRestoredStateScanServicesKey] as? [CBUUID] ?? []).map(\.uuidString),
                "scan_options": cb_optional(cb_restored_scan_options_payload(dict[CBCentralManagerRestoredStateScanOptionsKey] as? [String: Any])),
            ]
        }
    }

    func centralManager(
        _ central: CBCentralManager,
        didDiscover peripheral: CBPeripheral,
        advertisementData: [String: Any],
        rssi RSSI: NSNumber
    ) {
        sinks.broadcast("didDiscoverPeripheral") {
            [
                "peripheral_handle": cb_retained_handle(peripheral),
                "rssi": RSSI.intValue,
                "advertisement_data": advertisementData,
            ]
        }
    }

    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        sinks.broadcast("didConnectPeripheral") {
            ["peripheral_handle": cb_retained_handle(peripheral)]
        }
    }

    func centralManager(
        _ central: CBCentralManager,
        didFailToConnect peripheral: CBPeripheral,
        error: Error?
    ) {
        sinks.broadcast("didFailToConnectPeripheral") {
            [
                "peripheral_handle": cb_retained_handle(peripheral),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func centralManager(
        _ central: CBCentralManager,
        didDisconnectPeripheral peripheral: CBPeripheral,
        error: Error?
    ) {
        sinks.broadcast("didDisconnectPeripheral") {
            [
                "peripheral_handle": cb_retained_handle(peripheral),
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }

    func centralManager(
        _ central: CBCentralManager,
        didDisconnectPeripheral peripheral: CBPeripheral,
        timestamp: CFAbsoluteTime,
        isReconnecting: Bool,
        error: Error?
    ) {
        sinks.broadcast("didDisconnectPeripheral") {
            [
                "peripheral_handle": cb_retained_handle(peripheral),
                "timestamp": timestamp,
                "is_reconnecting": isReconnecting,
                "error": cb_optional(error.map(cb_error_object)),
            ]
        }
    }
}

private final class CBCentralManagerBox: NSObject {
    let manager: CBCentralManager
    let hub: CBCentralManagerHub
    let queue: DispatchQueue
    private var delegateSink: CBEventSink?

    init(
        manager: CBCentralManager,
        hub: CBCentralManagerHub,
        delegateSink: CBEventSink?,
        queue: DispatchQueue
    ) {
        self.manager = manager
        self.hub = hub
        self.delegateSink = delegateSink
        self.queue = queue
        super.init()
    }

    func detachDelegate() {
        guard let sink = delegateSink else { return }
        delegateSink = nil
        hub.sinks.remove(sink)
        sink.deactivate()
    }

    deinit {
        detachDelegate()
        manager.delegate = nil
    }
}

private func cb_manager_box(_ ptr: UnsafeMutableRawPointer?) -> CBCentralManagerBox? {
    guard let ptr else {
        return nil
    }
    let box: CBCentralManagerBox = cb_borrow(ptr)
    return box
}

@_cdecl("cb_manager_new")
public func cb_manager_new(
    _ optionsJSON: UnsafePointer<CChar>?,
    _ callback: CBCentralManagerEventCallback?,
    _ userInfo: UnsafeMutableRawPointer?,
    _ contextRetain: (@convention(c) (UnsafeMutableRawPointer?) -> Void)?,
    _ contextRelease: (@convention(c) (UnsafeMutableRawPointer?) -> Void)?,
    _ outManager: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    outManager.pointee = nil

    do {
        let payload = try cb_decode_json_if_present(optionsJSON, as: CBCentralManagerOptionsPayload.self) ?? CBCentralManagerOptionsPayload()
        let queue = DispatchQueue(label: payload.queue_label ?? "corebluetooth-rs.central")
        let hub = CBCentralManagerHub()
        let delegateSink = callback.map {
            CBEventSink(
                callback: $0,
                context: userInfo,
                retainContext: contextRetain,
                releaseContext: contextRelease,
                drainsOnDeactivate: false
            )
        }
        if let delegateSink {
            hub.sinks.add(delegateSink)
        }
        let manager = CBCentralManager(delegate: hub, queue: queue, options: cb_central_manager_options(payload))
        let box = CBCentralManagerBox(manager: manager, hub: hub, delegateSink: delegateSink, queue: queue)
        outManager.pointee = cb_retain(box)
        return CBR_OK
    } catch {
        cb_write_error(errorOut, error.localizedDescription)
        return CBR_INVALID_ARGUMENT
    }
}

@_cdecl("cb_manager_state")
public func cb_manager_state(_ managerPtr: UnsafeMutableRawPointer?) -> Int32 {
    Int32(cb_manager_box(managerPtr)?.manager.state.rawValue ?? CBManagerState.unknown.rawValue)
}

@_cdecl("cb_manager_authorization")
public func cb_manager_authorization(_ managerPtr: UnsafeMutableRawPointer?) -> Int32 {
    cb_manager_authorization_value(cb_manager_box(managerPtr)?.manager)
}

@_cdecl("cb_manager_global_authorization")
public func cb_manager_global_authorization() -> Int32 {
    if #available(macOS 10.15, *) {
        return Int32(CBManager.authorization.rawValue)
    }
    return 0
}

@_cdecl("cb_manager_is_scanning")
public func cb_manager_is_scanning(_ managerPtr: UnsafeMutableRawPointer?) -> Bool {
    cb_manager_box(managerPtr)?.manager.isScanning ?? false
}

@_cdecl("cb_manager_scan_for_peripherals")
public func cb_manager_scan_for_peripherals(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ serviceUUIDsJSON: UnsafePointer<CChar>?,
    _ scanOptionsJSON: UnsafePointer<CChar>?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let manager = cb_manager_box(managerPtr)?.manager else {
        cb_write_error(errorOut, "central manager must not be null")
        return CBR_INVALID_ARGUMENT
    }

    do {
        let uuids = try cb_service_uuids(serviceUUIDsJSON)
        let payload = try cb_decode_json_if_present(scanOptionsJSON, as: CBCentralManagerScanOptionsPayload.self)
        manager.scanForPeripherals(withServices: uuids, options: payload.flatMap(cb_scan_options))
        return CBR_OK
    } catch {
        cb_write_error(errorOut, error.localizedDescription)
        return CBR_INVALID_ARGUMENT
    }
}

@_cdecl("cb_manager_stop_scan")
public func cb_manager_stop_scan(_ managerPtr: UnsafeMutableRawPointer?) {
    cb_manager_box(managerPtr)?.manager.stopScan()
}

@_cdecl("cb_manager_connect")
public func cb_manager_connect(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ optionsJSON: UnsafePointer<CChar>?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard let manager = cb_manager_box(managerPtr)?.manager, let peripheral = cb_peripheral(peripheralPtr) else {
        cb_write_error(errorOut, "central manager and peripheral must not be null")
        return CBR_INVALID_ARGUMENT
    }

    do {
        let payload = try cb_decode_json_if_present(optionsJSON, as: CBCentralManagerConnectOptionsPayload.self) ?? CBCentralManagerConnectOptionsPayload()
        manager.connect(peripheral, options: cb_connect_options(payload))
        return CBR_OK
    } catch {
        cb_write_error(errorOut, error.localizedDescription)
        return CBR_INVALID_ARGUMENT
    }
}

@_cdecl("cb_manager_cancel_peripheral_connection")
public func cb_manager_cancel_peripheral_connection(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ peripheralPtr: UnsafeMutableRawPointer?
) {
    guard let manager = cb_manager_box(managerPtr)?.manager, let peripheral = cb_peripheral(peripheralPtr) else {
        return
    }

    manager.cancelPeripheralConnection(peripheral)
}

@_cdecl("cb_manager_retrieve_connected_peripherals")
public func cb_manager_retrieve_connected_peripherals(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ serviceUUIDsJSON: UnsafePointer<CChar>?,
    _ outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outCount: UnsafeMutablePointer<Int>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    outArray.pointee = nil
    outCount.pointee = 0
    guard let manager = cb_manager_box(managerPtr)?.manager else {
        cb_write_error(errorOut, "central manager must not be null")
        return CBR_INVALID_ARGUMENT
    }

    do {
        let services = try cb_service_uuids(serviceUUIDsJSON) ?? []
        cb_make_pointer_array(manager.retrieveConnectedPeripherals(withServices: services), outArray, outCount)
        return CBR_OK
    } catch {
        cb_write_error(errorOut, error.localizedDescription)
        return CBR_INVALID_ARGUMENT
    }
}

@_cdecl("cb_manager_retrieve_peripherals_with_identifiers")
public func cb_manager_retrieve_peripherals_with_identifiers(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ identifiersJSON: UnsafePointer<CChar>?,
    _ outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outCount: UnsafeMutablePointer<Int>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    outArray.pointee = nil
    outCount.pointee = 0
    guard let manager = cb_manager_box(managerPtr)?.manager else {
        cb_write_error(errorOut, "central manager must not be null")
        return CBR_INVALID_ARGUMENT
    }

    do {
        let identifiers = try cb_identifier_uuids(identifiersJSON) ?? []
        cb_make_pointer_array(manager.retrievePeripherals(withIdentifiers: identifiers), outArray, outCount)
        return CBR_OK
    } catch {
        cb_write_error(errorOut, error.localizedDescription)
        return CBR_INVALID_ARGUMENT
    }
}

@_cdecl("cb_manager_detach_delegate")
public func cb_manager_detach_delegate(_ managerPtr: UnsafeMutableRawPointer?) {
    cb_manager_box(managerPtr)?.detachDelegate()
}

func cb_central_manager_hub(_ ptr: UnsafeMutableRawPointer?) -> CBCentralManagerHub? {
    cb_manager_box(ptr)?.hub
}
