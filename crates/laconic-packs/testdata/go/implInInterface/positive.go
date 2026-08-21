package implininterface

type internalCache struct{}

// Get reads through internalCache when the entry is cold.
func Get(k string) string {
	return k
}
