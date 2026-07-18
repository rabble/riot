import CoreImage
import CoreImage.CIFilterBuiltins
import CoreGraphics

/// Renders a string into a QR-code raster locally — no network, no external
/// service, no fabricated content. Pure CoreImage (`CIQRCodeGenerator`), so it
/// compiles and runs on BOTH iOS and macOS and is unit-testable without a screen.
/// Returns a platform-neutral `CGImage`; the SwiftUI layer wraps it in
/// `Image(decorative:scale:)`. A blank input or a CoreImage failure yields `nil`,
/// so the caller renders an honest "nothing to share yet" state, never a broken
/// image.
public enum QRImageRenderer {
    /// `correctionLevel` "M" (~15% recovery) balances density against scan
    /// resilience for a `riot://` join payload; `scale` nearest-neighbour-magnifies
    /// the raw ~1-module-per-pixel matrix so the code is crisp at display size.
    public static func makeQRCode(from string: String, scale: CGFloat = 12) -> CGImage? {
        let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let payload = trimmed.data(using: .utf8) else { return nil }

        let filter = CIFilter.qrCodeGenerator()
        filter.message = payload
        filter.correctionLevel = "M"
        guard let output = filter.outputImage else { return nil }

        let scaled = output.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
        return CIContext().createCGImage(scaled, from: scaled.extent)
    }
}
