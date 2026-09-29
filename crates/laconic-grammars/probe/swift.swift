// a line comment

/* a block comment */

// MARK: - Probe

/// Documents an exported function.
public func exported(_ a: Int) -> Int {
    let x = a + 1 // a trailing comment
    print(x)

    // a detached comment

    return x
}

/**
 * Documents an exported struct.
 */
public struct Probe {
    /// Documents a computed property.
    public var doubled: Int { 2 }

    private func hidden() {}
}
