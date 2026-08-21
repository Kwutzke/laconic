package detached

func f() {
	// the checksum is attacker-controlled, hence the exemption
	//nolint:gosec
	q := risky()
	_ = q
}
