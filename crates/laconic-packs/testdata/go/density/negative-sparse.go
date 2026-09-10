package density

func sparse(items []string) map[string]int {
	counts := make(map[string]int, len(items))
	for _, item := range items {
		counts[item]++
	}
	total := 0
	for _, n := range counts {
		total += n
	}
	// An empty input still returns the map rather than nil, because callers
	// range over the result without checking it first.
	if total == 0 {
		return counts
	}
	return counts
}
