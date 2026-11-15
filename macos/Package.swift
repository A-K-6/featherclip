// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "FeatherClip", platforms: [.macOS(.v13)],
    products: [.executable(name: "FeatherClip", targets: ["FeatherClip"])],
    targets: [
        .target(name: "FeatherClipCore"),
        .executableTarget(name: "FeatherClip", dependencies: ["FeatherClipCore"]),
        .testTarget(name: "FeatherClipCoreTests", dependencies: ["FeatherClipCore"])
    ], swiftLanguageVersions: [.v5]
)
