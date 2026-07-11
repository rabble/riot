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
