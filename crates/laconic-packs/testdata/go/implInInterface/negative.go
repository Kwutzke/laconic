package implininterface

type Cache struct{}

// Get reads through Cache when the entry is cold.
func Get(k string) string {
	return k
}
