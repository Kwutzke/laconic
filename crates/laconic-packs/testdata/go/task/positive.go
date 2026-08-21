package task

func f() {
	// TODO fix the parser
	x := 1
	_ = x

	// FIXME leaks a file handle on the error path
	y := 2
	_ = y
}
