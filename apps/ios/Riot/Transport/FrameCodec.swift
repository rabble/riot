import Foundation

struct FrameDecoder {
    private var buffer = Data()
    private let maxFrameBytes: Int

    init(maxFrameBytes: Int = NearbyLimits.maxFrameBytes) {
        self.maxFrameBytes = maxFrameBytes
    }

    mutating func append(_ bytes: Data) throws -> [Data] {
        buffer.append(bytes)
        var frames: [Data] = []
        while buffer.count >= 4 {
            let length = buffer.prefix(4).reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
            guard Int(length) <= maxFrameBytes else { throw NearbyTransportError.disconnected }
            let total = 4 + Int(length)
            guard buffer.count >= total else { break }
            frames.append(buffer.subdata(in: 4..<total))
            buffer.removeSubrange(0..<total)
        }
        return frames
    }

    static func encode(_ frame: Data, maxFrameBytes: Int = NearbyLimits.maxFrameBytes) throws -> Data {
        guard frame.count <= maxFrameBytes, frame.count <= Int(UInt32.max) else {
            throw NearbyTransportError.disconnected
        }
        let count = UInt32(frame.count).bigEndian
        var bytes = Data(bytes: [count], count: 4)
        bytes.append(frame)
        return bytes
    }
}

/// The one frame two phones exchange before they sync: "here is the space I am
/// in", or "I am not in one".
///
/// It is a whole frame in its own right — the channels below already length-
/// prefix what is handed to them — and it is the FIRST thing either side sends,
/// before any sync frame exists, so there is no ambiguity with the core's own
/// frames on the wire.
///
/// The decoder is deliberately strict. These bytes come from a stranger's phone
/// and the namespace in them can end up being the space this person joins, so
/// anything that is not exactly one well-formed announce is refused rather than
/// interpreted: unknown magic, unknown flag, a namespace that is not 32 bytes, a
/// title that is empty or over Rust's own 512-byte cap, invalid UTF-8, or so
/// much as one trailing byte.
enum SpaceAnnounceCodec {
    static let magic = Data("RIOTSP01".utf8)
    static let maxTitleBytes = 512
    static let namespaceBytes = 32

    static func encode(_ space: RiotSpace?) throws -> Data {
        var bytes = magic
        guard let space else {
            bytes.append(0)
            return bytes
        }
        let namespace = try hexBytes(space.namespaceID)
        let title = Data(space.title.utf8)
        guard !title.isEmpty, title.count <= maxTitleBytes else {
            throw NearbyTransportError.disconnected
        }
        bytes.append(1)
        bytes.append(namespace)
        bytes.append(UInt8(title.count >> 8))
        bytes.append(UInt8(title.count & 0xFF))
        bytes.append(title)
        return bytes
    }

    /// The peer's space, or nil if they announced that they are not in one.
    /// Throws on anything malformed.
    static func decode(_ frame: Data) throws -> RiotSpace? {
        var rest = Data(frame)
        guard rest.count > magic.count, rest.prefix(magic.count) == magic else {
            throw NearbyTransportError.disconnected
        }
        rest = rest.dropFirst(magic.count)
        let flag = rest.removeFirst()
        if flag == 0 {
            guard rest.isEmpty else { throw NearbyTransportError.disconnected }
            return nil
        }
        guard flag == 1, rest.count >= namespaceBytes + 2 else {
            throw NearbyTransportError.disconnected
        }
        let namespace = rest.prefix(namespaceBytes)
        rest = rest.dropFirst(namespaceBytes)
        let length = Int(rest.removeFirst()) << 8 | Int(rest.removeFirst())
        guard length > 0, length <= maxTitleBytes, rest.count == length else {
            throw NearbyTransportError.disconnected
        }
        guard let title = String(data: rest, encoding: .utf8),
              !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw NearbyTransportError.disconnected
        }
        return RiotSpace(namespaceID: hexString(namespace), title: title)
    }

    /// The namespace travels as its 32 raw bytes, not as text, so there is one
    /// encoding of a given space on the wire and a malformed id cannot reach
    /// Rust's parser at all.
    private static func hexBytes(_ hex: String) throws -> Data {
        let characters = Array(hex.utf8)
        guard characters.count == namespaceBytes * 2 else { throw NearbyTransportError.disconnected }
        var bytes = Data(capacity: namespaceBytes)
        for pair in stride(from: 0, to: characters.count, by: 2) {
            guard let high = nibble(characters[pair]), let low = nibble(characters[pair + 1]) else {
                throw NearbyTransportError.disconnected
            }
            bytes.append(high << 4 | low)
        }
        return bytes
    }

    private static func nibble(_ character: UInt8) -> UInt8? {
        switch character {
        case 0x30...0x39: character - 0x30
        case 0x61...0x66: character - 0x61 + 10
        case 0x41...0x46: character - 0x41 + 10
        default: nil
        }
    }

    private static func hexString(_ bytes: Data) -> String {
        bytes.map { String(format: "%02x", $0) }.joined()
    }
}
