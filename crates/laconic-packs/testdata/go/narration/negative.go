package narration

func f() {
	// the map is keyed by the canonical name, not the display name
	m := map[string]int{}
	_ = m

	// previouslyKnownAs is the legacy field and must stay in the payload
	n := 1
	_ = n
}
