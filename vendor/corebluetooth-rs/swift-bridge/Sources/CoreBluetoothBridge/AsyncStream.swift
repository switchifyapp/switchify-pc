import CoreBluetooth
import Foundation

private func cb_stream_sink(
    _ onEvent: @escaping CBEventCallback,
    _ ctx: UnsafeMutableRawPointer?,
    _ retainCtx: CBContextCallback?,
    _ releaseCtx: CBContextCallback?
) -> CBEventSink {
    CBEventSink(
        callback: onEvent,
        context: ctx,
        retainContext: retainCtx,
        releaseContext: releaseCtx,
        drainsOnDeactivate: true,
        ignoredEvents: ["willRestoreState"]
    )
}

private func cb_stream_unsubscribe(_ sinkPtr: UnsafeMutableRawPointer?, from sinks: CBEventSinkList?) {
    guard let sinkPtr else { return }
    let sink = Unmanaged<CBEventSink>.fromOpaque(sinkPtr).takeRetainedValue()
    sinks?.remove(sink)
    sink.deactivate()
}

@_cdecl("cb_central_manager_stream_subscribe")
public func cb_central_manager_stream_subscribe(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ onEvent: @escaping CBEventCallback,
    _ ctx: UnsafeMutableRawPointer?,
    _ retainCtx: CBContextCallback?,
    _ releaseCtx: CBContextCallback?
) -> UnsafeMutableRawPointer? {
    guard let hub = cb_central_manager_hub(managerPtr) else { return nil }
    let sink = cb_stream_sink(onEvent, ctx, retainCtx, releaseCtx)
    hub.sinks.add(sink)
    return cb_retain(sink)
}

@_cdecl("cb_central_manager_stream_unsubscribe")
public func cb_central_manager_stream_unsubscribe(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ sinkPtr: UnsafeMutableRawPointer?
) {
    cb_stream_unsubscribe(sinkPtr, from: cb_central_manager_hub(managerPtr)?.sinks)
}

@_cdecl("cb_peripheral_stream_subscribe")
public func cb_peripheral_stream_subscribe(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ onEvent: @escaping CBEventCallback,
    _ ctx: UnsafeMutableRawPointer?,
    _ retainCtx: CBContextCallback?,
    _ releaseCtx: CBContextCallback?
) -> UnsafeMutableRawPointer? {
    guard let peripheral = cb_peripheral(peripheralPtr) else { return nil }
    let sink = cb_stream_sink(onEvent, ctx, retainCtx, releaseCtx)
    cb_peripheral_hub(peripheral).streams.add(sink)
    return cb_retain(sink)
}

@_cdecl("cb_peripheral_stream_unsubscribe")
public func cb_peripheral_stream_unsubscribe(
    _ peripheralPtr: UnsafeMutableRawPointer?,
    _ sinkPtr: UnsafeMutableRawPointer?
) {
    cb_stream_unsubscribe(sinkPtr, from: cb_peripheral(peripheralPtr).map { cb_peripheral_hub($0).streams })
}

@_cdecl("cb_peripheral_manager_stream_subscribe")
public func cb_peripheral_manager_stream_subscribe(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ onEvent: @escaping CBEventCallback,
    _ ctx: UnsafeMutableRawPointer?,
    _ retainCtx: CBContextCallback?,
    _ releaseCtx: CBContextCallback?
) -> UnsafeMutableRawPointer? {
    guard let hub = cb_peripheral_manager_hub(managerPtr) else { return nil }
    let sink = cb_stream_sink(onEvent, ctx, retainCtx, releaseCtx)
    hub.sinks.add(sink)
    return cb_retain(sink)
}

@_cdecl("cb_peripheral_manager_stream_unsubscribe")
public func cb_peripheral_manager_stream_unsubscribe(
    _ managerPtr: UnsafeMutableRawPointer?,
    _ sinkPtr: UnsafeMutableRawPointer?
) {
    cb_stream_unsubscribe(sinkPtr, from: cb_peripheral_manager_hub(managerPtr)?.sinks)
}
