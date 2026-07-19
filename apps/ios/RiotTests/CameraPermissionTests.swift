import XCTest
@testable import RiotKit

/// Unit 1 — the camera-denied recovery for the QR scan path. The live capture in
/// `QRScannerView` is not unit-testable, but the recovery copy + Settings deep link
/// are: they must speak about the CAMERA (this is the scan permission), not reuse
/// the Nearby flow's Bluetooth wording.
final class CameraPermissionTests: XCTestCase {
    func testRecoveryCarriesCameraCopyAndSettingsURL() {
        XCTAssertTrue(
            CameraPermissionRecovery.message.localizedCaseInsensitiveContains("camera"),
            "the camera-denied recovery must name the camera"
        )
        XCTAssertNotNil(
            CameraPermissionRecovery.settingsURL,
            "the recovery offers a deep link into Settings"
        )
        XCTAssertFalse(
            CameraPermissionRecovery.message.localizedCaseInsensitiveContains("bluetooth"),
            "this is the camera permission, not the Nearby Bluetooth one"
        )
    }

    /// The recovery keeps paste alive: it must not tell the person the only way in is
    /// the camera.
    func testRecoveryPreservesThePasteFallback() {
        XCTAssertTrue(
            CameraPermissionRecovery.message.localizedCaseInsensitiveContains("paste"),
            "camera denied still leaves paste-a-link available; the copy says so"
        )
    }
}
