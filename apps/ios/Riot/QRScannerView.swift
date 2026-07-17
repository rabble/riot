import SwiftUI

#if os(iOS)
import AVFoundation
import UIKit

/// A live QR scanner for the join-by-reference flow. iOS-only: macOS has no camera
/// target and joins by paste. Wraps an `AVCaptureSession` reading `.qr` metadata and
/// hands each decoded string to `onScanned` — it does NOT interpret the payload; the
/// `riot://`-scheme + length filtering lives in
/// ``JoinReferenceModel/preview(fromScannedString:)`` so hostile QR input is
/// validated in one tested place.
///
/// Hardening:
/// - The session is started only after camera authorization is granted (requesting
///   it if undetermined); denied/restricted shows the recovery card and NEVER starts
///   capture.
/// - The session is torn down on `viewWillDisappear` and when the app backgrounds —
///   no lingering capture after the sheet is dismissed.
public struct QRScannerView: UIViewControllerRepresentable {
    private let onScanned: (String) -> Void
    private let onPermissionDenied: () -> Void

    public init(
        onScanned: @escaping (String) -> Void,
        onPermissionDenied: @escaping () -> Void = {}
    ) {
        self.onScanned = onScanned
        self.onPermissionDenied = onPermissionDenied
    }

    public func makeUIViewController(context: Context) -> QRScannerViewController {
        let controller = QRScannerViewController()
        controller.onScanned = onScanned
        controller.onPermissionDenied = onPermissionDenied
        return controller
    }

    public func updateUIViewController(_ controller: QRScannerViewController, context: Context) {}
}

/// The capture controller behind ``QRScannerView``. Public so the representable can
/// name it; not intended for direct use.
public final class QRScannerViewController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    var onScanned: ((String) -> Void)?
    var onPermissionDenied: (() -> Void)?

    private let session = AVCaptureSession()
    private var previewLayer: AVCaptureVideoPreviewLayer?
    /// Latches so a single scan fires the callback once, not on every frame the code
    /// stays in view.
    private var didScan = false

    public override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        configureForAuthorization()
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(stopCapture),
            name: UIApplication.didEnterBackgroundNotification,
            object: nil
        )
    }

    private func configureForAuthorization() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configureSession()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
                DispatchQueue.main.async {
                    guard let self else { return }
                    if granted {
                        self.configureSession()
                    } else {
                        self.onPermissionDenied?()
                    }
                }
            }
        case .denied, .restricted:
            onPermissionDenied?()
        @unknown default:
            onPermissionDenied?()
        }
    }

    private func configureSession() {
        guard
            let device = AVCaptureDevice.default(for: .video),
            let input = try? AVCaptureDeviceInput(device: device),
            session.canAddInput(input)
        else {
            onPermissionDenied?()
            return
        }
        session.addInput(input)

        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else {
            onPermissionDenied?()
            return
        }
        session.addOutput(output)
        // Metadata is delivered on the main queue, which is why the nonisolated
        // delegate method below can safely assume main-actor isolation.
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = [.qr]

        let preview = AVCaptureVideoPreviewLayer(session: session)
        preview.videoGravity = .resizeAspectFill
        preview.frame = view.layer.bounds
        view.layer.addSublayer(preview)
        previewLayer = preview

        startCapture()
    }

    public override func viewWillLayoutSubviews() {
        super.viewWillLayoutSubviews()
        previewLayer?.frame = view.layer.bounds
    }

    public override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        // Resume only when the session is already configured (permission granted).
        if !session.inputs.isEmpty { startCapture() }
    }

    public override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        stopCapture()
    }

    /// Nonisolated so the `@MainActor` controller can satisfy the nonisolated
    /// `AVCaptureMetadataOutputObjectsDelegate` requirement under Swift 6. The
    /// delegate queue is `.main`, so hopping onto the main actor is provably safe.
    public nonisolated func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection
    ) {
        guard
            let object = metadataObjects.first as? AVMetadataMachineReadableCodeObject,
            object.type == .qr,
            let value = object.stringValue
        else { return }
        MainActor.assumeIsolated { handleScan(value) }
    }

    private func handleScan(_ value: String) {
        guard !didScan else { return }
        didScan = true
        stopCapture()
        onScanned?(value)
    }

    private func startCapture() {
        guard !session.isRunning else { return }
        session.startRunning()
    }

    @objc private func stopCapture() {
        guard session.isRunning else { return }
        session.stopRunning()
    }
}
#endif
