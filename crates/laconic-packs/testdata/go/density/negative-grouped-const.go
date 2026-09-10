package density

const (
	// ModeRequired rejects a request carrying no token.
	ModeRequired = iota
	// ModeOptional lets an anonymous caller through to the handler.
	ModeOptional
)
