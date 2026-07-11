import Foundation

struct FrameDecoder {
    private var buffer = Data()

    mutating func append(_ bytes: Data) throws -> [Data] {
        buffer.append(bytes)
        var frames: [Data] = []
        while buffer.count >= 4 {
            let length = buffer.prefix(4).reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
            guard length <= 1_048_576 else { throw NearbyTransportError.disconnected }
            let total = 4 + Int(length)
            guard buffer.count >= total else { break }
            frames.append(buffer.subdata(in: 4..<total))
            buffer.removeSubrange(0..<total)
        }
        return frames
    }

    static func encode(_ frame: Data) -> Data {
        let count = UInt32(frame.count).bigEndian
        var bytes = Data(bytes: [count], count: 4)
        bytes.append(frame)
        return bytes
    }
}
