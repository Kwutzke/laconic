package docbloat

// Config holds the parser settings.
//
// The zero value is not usable and callers must set N, which this paragraph
// explains at more length than the type deserves. It is here to run past three
// times the body it documents, because that is what the relative test measures.
// A type declaration names no fields in the grammar at all, so reading a field
// left the relative test dead on every Go type, const and var — the whole set
// this pack deliberately makes documentable, and the reason the measurement is
// the pack's answer rather than the engine's guess about where a body lives.
// This block is over the cap as well as over the ratio, so it is not evidence for the ratio
// alone; a_one_method_interface_is_a_denominator is where that case is isolated.
type Config struct {
	N int
}
