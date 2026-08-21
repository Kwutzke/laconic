package narration

func f() {
	// changed to use a map for lookups
	m := map[string]int{}
	_ = m

	// Previously this returned an error.
	n := 1
	_ = n
}
