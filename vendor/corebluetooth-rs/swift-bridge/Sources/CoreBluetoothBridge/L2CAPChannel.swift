import CoreBluetooth
import Foundation

@_cdecl("cb_l2cap_channel_peer")
public func cb_l2cap_channel_peer(_ channelPtr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    cb_l2cap_channel(channelPtr).map { cb_retain($0.peer) }
}

@_cdecl("cb_l2cap_channel_psm")
public func cb_l2cap_channel_psm(_ channelPtr: UnsafeMutableRawPointer?) -> UInt16 {
    cb_l2cap_channel(channelPtr)?.psm ?? 0
}

@_cdecl("cb_l2cap_channel_input_stream")
public func cb_l2cap_channel_input_stream(_ channelPtr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    cb_l2cap_channel(channelPtr).map { cb_retain($0.inputStream) }
}

@_cdecl("cb_l2cap_channel_output_stream")
public func cb_l2cap_channel_output_stream(_ channelPtr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    cb_l2cap_channel(channelPtr).map { cb_retain($0.outputStream) }
}

@_cdecl("cb_peer_identifier")
public func cb_peer_identifier(_ peerPtr: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    cb_peer(peerPtr).flatMap { cb_string($0.identifier.uuidString) }
}

@_cdecl("cb_stream_status")
public func cb_stream_status(_ streamPtr: UnsafeMutableRawPointer?) -> Int32 {
    Int32(cb_stream(streamPtr)?.streamStatus.rawValue ?? Stream.Status.notOpen.rawValue)
}

@_cdecl("cb_input_stream_has_bytes_available")
public func cb_input_stream_has_bytes_available(_ streamPtr: UnsafeMutableRawPointer?) -> Bool {
    cb_input_stream(streamPtr)?.hasBytesAvailable ?? false
}

@_cdecl("cb_output_stream_has_space_available")
public func cb_output_stream_has_space_available(_ streamPtr: UnsafeMutableRawPointer?) -> Bool {
    cb_output_stream(streamPtr)?.hasSpaceAvailable ?? false
}

@_cdecl("cb_stream_open")
public func cb_stream_open(_ streamPtr: UnsafeMutableRawPointer?) {
    cb_stream(streamPtr)?.open()
}

@_cdecl("cb_stream_close")
public func cb_stream_close(_ streamPtr: UnsafeMutableRawPointer?) {
    cb_stream(streamPtr)?.close()
}
