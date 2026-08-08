import CoreBluetooth
import Foundation

struct CBAdvertisementPayload: Codable {
    var local_name: String?
    var service_uuids: [String] = []
}

func cb_advertisement_dictionary(_ cString: UnsafePointer<CChar>?) throws -> [String: Any]? {
    guard let payload = try cb_decode_json_if_present(cString, as: CBAdvertisementPayload.self) else {
        return nil
    }

    var advertisement: [String: Any] = [:]
    if let localName = payload.local_name {
        advertisement[CBAdvertisementDataLocalNameKey] = localName
    }
    if !payload.service_uuids.isEmpty {
        advertisement[CBAdvertisementDataServiceUUIDsKey] = payload.service_uuids.map(CBUUID.init(string:))
    }
    return advertisement.isEmpty ? nil : advertisement
}
