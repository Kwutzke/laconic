package restate

import "strings"

func normalize(name string) string { return strings.ToLower(name) }

var ( // name
	defaultName = "x"
)

func loop(items []string) {
	for _, item := range items {
		trim(item)
	} // trim item
}
