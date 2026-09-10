package density

// Documented reports how many items were supplied. The result is meaningful at
// zero, so callers distinguish an absent key from a zero count.
func Documented(items []string) int {
	n := 0
	for range items {
		n++
	}
	return n
}
