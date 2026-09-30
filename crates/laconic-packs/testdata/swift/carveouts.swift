// swiftlint:disable:next force_cast
// swift-format-ignore: NeverForceUnwrap
// swiftformat:disable redundantSelf
// sourcery: autoMockable
// periphery:ignore - kept for the plugin API
// MARK: - Probe

/// Documents an exported function.
public func exported(_ a: Int) -> Int {
    let x = a + 1
    return x
}

public protocol Source {
    /// Documents a requirement.
    func poll()
}

private func hidden() {}
