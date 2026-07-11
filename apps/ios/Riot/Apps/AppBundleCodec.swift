import Foundation

/// One served resource: an in-bundle path, its content type, and its bytes.
public struct AppResource: Equatable, Sendable {
    public let path: String
    public let contentType: String
    public let bytes: Data

    public init(path: String, contentType: String, bytes: Data) {
        self.path = path
        self.contentType = contentType
        self.bytes = bytes
    }
}

/// A decoded app bundle: a primary entry point plus its resource list.
public struct DecodedAppBundle: Equatable, Sendable {
    public let entryPoint: String
    public let resources: [AppResource]

    public init(entryPoint: String, resources: [AppResource]) {
        self.entryPoint = entryPoint
        self.resources = resources
    }
}

public struct AppBundleCodecError: Error, Equatable {
    public let message: String
    public init(_ message: String) { self.message = message }
}

/// Minimal canonical CBOR major types — the exact subset the bundle format
/// uses.
private enum CborMajor {
    static let uint = 0
    static let bytes = 2
    static let text = 3
    static let array = 4
    static let map = 5
}

/// Emits definite-length, minimal-head CBOR items.
private struct CborWriter {
    private(set) var bytes: [UInt8] = []

    mutating func head(_ major: Int, _ value: UInt64) {
        let mt = UInt8(major << 5)
        switch value {
        case ..<24:
            bytes.append(mt | UInt8(value))
        case ..<0x100:
            bytes.append(mt | 24)
            appendBigEndian(value, 1)
        case ..<0x1_0000:
            bytes.append(mt | 25)
            appendBigEndian(value, 2)
        case ..<0x1_0000_0000:
            bytes.append(mt | 26)
            appendBigEndian(value, 4)
        default:
            bytes.append(mt | 27)
            appendBigEndian(value, 8)
        }
    }

    private mutating func appendBigEndian(_ value: UInt64, _ count: Int) {
        for shift in stride(from: count - 1, through: 0, by: -1) {
            bytes.append(UInt8((value >> (shift * 8)) & 0xFF))
        }
    }

    mutating func uint(_ value: UInt64) { head(CborMajor.uint, value) }
    mutating func map(_ entries: Int) { head(CborMajor.map, UInt64(entries)) }
    mutating func array(_ items: Int) { head(CborMajor.array, UInt64(items)) }

    mutating func text(_ value: String) {
        let encoded = Array(value.utf8)
        head(CborMajor.text, UInt64(encoded.count))
        bytes.append(contentsOf: encoded)
    }

    mutating func byteString(_ value: Data) {
        head(CborMajor.bytes, UInt64(value.count))
        bytes.append(contentsOf: value)
    }
}

/// Strict cursor reader. Rejects indefinite/reserved length forms and reads
/// bounds *before* consuming input sized from them, so a forged length can
/// never trigger an oversized allocation. Non-minimal or otherwise
/// non-canonical encodings survive the read but are rejected by the codec's
/// re-encode proof.
private struct CborReader {
    private let input: [UInt8]
    private(set) var position: Int = 0

    init(_ input: [UInt8]) { self.input = input }

    var size: Int { input.count }

    private mutating func readByte() throws -> Int {
        guard position < input.count else {
            throw AppBundleCodecError("unexpected end of CBOR input")
        }
        defer { position += 1 }
        return Int(input[position])
    }

    private mutating func readUIntBytes(_ count: Int) throws -> UInt64 {
        var value: UInt64 = 0
        for _ in 0..<count { value = (value << 8) | UInt64(try readByte()) }
        return value
    }

    /// Reads one item head; rejects indefinite (31) and reserved (28-30).
    private mutating func readHead() throws -> (major: Int, argument: UInt64) {
        let initial = try readByte()
        let major = initial >> 5
        let info = initial & 0x1F
        let argument: UInt64
        switch info {
        case ..<24: argument = UInt64(info)
        case 24: argument = try readUIntBytes(1)
        case 25: argument = try readUIntBytes(2)
        case 26: argument = try readUIntBytes(4)
        case 27: argument = try readUIntBytes(8)
        default: throw AppBundleCodecError("indefinite or reserved CBOR length")
        }
        return (major, argument)
    }

    mutating func expectMap(_ entries: Int) throws {
        let (major, argument) = try readHead()
        guard major == CborMajor.map, argument == UInt64(entries) else {
            throw AppBundleCodecError("expected map(\(entries))")
        }
    }

    mutating func readArrayHeader() throws -> UInt64 {
        let (major, argument) = try readHead()
        guard major == CborMajor.array else {
            throw AppBundleCodecError("expected array")
        }
        return argument
    }

    mutating func expectUInt(_ value: UInt64) throws {
        let (major, argument) = try readHead()
        guard major == CborMajor.uint, argument == value else {
            throw AppBundleCodecError("expected uint \(value)")
        }
    }

    mutating func readText(_ maxBytes: Int) throws -> String {
        let (major, argument) = try readHead()
        guard major == CborMajor.text else {
            throw AppBundleCodecError("expected text")
        }
        guard argument != 0, argument <= UInt64(maxBytes) else {
            throw AppBundleCodecError("text length out of bounds")
        }
        let length = Int(argument)
        guard position + length <= input.count else {
            throw AppBundleCodecError("text overruns CBOR input")
        }
        guard let text = String(bytes: input[position..<position + length], encoding: .utf8) else {
            throw AppBundleCodecError("invalid UTF-8 in text")
        }
        position += length
        return text
    }

    mutating func readBytes(_ maxBytes: Int) throws -> Data {
        let (major, argument) = try readHead()
        guard major == CborMajor.bytes else {
            throw AppBundleCodecError("expected bytes")
        }
        guard argument <= UInt64(maxBytes) else {
            throw AppBundleCodecError("byte string length out of bounds")
        }
        let length = Int(argument)
        guard position + length <= input.count else {
            throw AppBundleCodecError("byte string overruns CBOR input")
        }
        let value = Data(input[position..<position + length])
        position += length
        return value
    }
}

/// Strict Swift mirror of `crates/riot-core/src/apps/bundle.rs` (and of
/// Android's `AppBundleCodec.kt`): the same map/key layout
/// (`map(2){0: entry_point, 1: [map(3){0: path, 1: content_type, 2: bytes}]}`),
/// the same bounds, and the same canonicality proof (decode re-encodes and
/// compares byte-for-byte).
///
/// This is a *serving* mirror, not a security boundary: production only ever
/// decodes bytes that Rust's `install_app` already accepted. Any drift from
/// Rust surfaces as a loud install failure, never as a silently divergent
/// decode.
public enum AppBundleCodec {
    public static let maxBundleResources = 32
    public static let maxResourcePathBytes = 256
    public static let maxResourceContentTypeBytes = 64
    public static let maxBundleTotalBytes = 1_048_576

    public static func encode(_ bundle: DecodedAppBundle) throws -> Data {
        try validate(bundle)

        var writer = CborWriter()
        writer.map(2)
        writer.uint(0)
        writer.text(bundle.entryPoint)
        writer.uint(1)
        writer.array(bundle.resources.count)
        for resource in bundle.resources {
            writer.map(3)
            writer.uint(0)
            writer.text(resource.path)
            writer.uint(1)
            writer.text(resource.contentType)
            writer.uint(2)
            writer.byteString(resource.bytes)
        }

        let encoded = Data(writer.bytes)
        if encoded.count > maxBundleTotalBytes {
            throw AppBundleCodecError("encoded bundle exceeds size limit")
        }
        return encoded
    }

    public static func decode(_ input: Data) throws -> DecodedAppBundle {
        if input.count > maxBundleTotalBytes {
            throw AppBundleCodecError("bundle exceeds size limit")
        }

        var reader = CborReader([UInt8](input))
        try reader.expectMap(2)

        try reader.expectUInt(0)
        let entryPoint = try reader.readText(maxResourcePathBytes)

        try reader.expectUInt(1)
        let resourceCount = try reader.readArrayHeader()
        if resourceCount == 0 || resourceCount > UInt64(maxBundleResources) {
            throw AppBundleCodecError("resource count out of bounds")
        }

        var resources: [AppResource] = []
        resources.reserveCapacity(Int(resourceCount))
        for _ in 0..<resourceCount {
            try reader.expectMap(3)

            try reader.expectUInt(0)
            let path = try reader.readText(maxResourcePathBytes)

            try reader.expectUInt(1)
            let contentType = try reader.readText(maxResourceContentTypeBytes)

            try reader.expectUInt(2)
            let bytes = try reader.readBytes(maxBundleTotalBytes)

            resources.append(AppResource(path: path, contentType: contentType, bytes: bytes))
        }

        if reader.position != reader.size {
            throw AppBundleCodecError("trailing bytes after bundle")
        }

        let bundle = DecodedAppBundle(entryPoint: entryPoint, resources: resources)
        try validate(bundle)

        // Canonicality proof: only the exact encoder output is acceptable.
        if try encode(bundle) != input {
            throw AppBundleCodecError("non-canonical bundle encoding")
        }
        return bundle
    }

    private static func validate(_ bundle: DecodedAppBundle) throws {
        if bundle.resources.isEmpty || bundle.resources.count > maxBundleResources {
            throw AppBundleCodecError("resource count out of bounds")
        }

        var totalBytes = 0
        var entryPointFound = false
        for resource in bundle.resources {
            if resource.path.isEmpty || resource.path.utf8.count > maxResourcePathBytes {
                throw AppBundleCodecError("resource path out of bounds")
            }
            if resource.contentType.isEmpty
                || resource.contentType.utf8.count > maxResourceContentTypeBytes {
                throw AppBundleCodecError("resource content type out of bounds")
            }
            if resource.path == bundle.entryPoint {
                entryPointFound = true
            }
            totalBytes += resource.bytes.count
        }

        if !entryPointFound {
            throw AppBundleCodecError("entry point not present among resources")
        }
        if totalBytes > maxBundleTotalBytes {
            throw AppBundleCodecError("bundle exceeds size limit")
        }
    }
}
