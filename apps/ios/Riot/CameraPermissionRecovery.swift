import Foundation
#if canImport(UIKit)
import UIKit
#endif

/// The recovery shown when camera access is denied on the QR scan path — the exact
/// analog of ``NearbyPermissionRecovery`` for the camera. A plain-language
/// explanation plus a deep link into Settings, and it deliberately keeps the PASTE
/// fallback in view so a denied camera is never a dead end. Modeled as a value type
/// so the copy + Settings URL are testable without a camera.
public enum CameraPermissionRecovery {
    public static var settingsURL: URL? {
        #if canImport(UIKit)
        return URL(string: UIApplication.openSettingsURLString)
        #else
        return URL(string: "x-apple.systempreferences:")
        #endif
    }

    public static let message =
        "Riot needs camera access to scan a community's QR code. "
        + "You can still paste a link instead. Open Settings to turn the camera on."
}
