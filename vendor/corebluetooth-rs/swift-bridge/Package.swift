// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "CoreBluetoothBridge",
    platforms: [
        .macOS(.v10_13)
    ],
    products: [
        .library(
            name: "CoreBluetoothBridge",
            type: .static,
            targets: ["CoreBluetoothBridge"]
        )
    ],
    targets: [
        .target(
            name: "CoreBluetoothBridge",
            path: "Sources/CoreBluetoothBridge",
            publicHeadersPath: "include"
        )
    ]
)
