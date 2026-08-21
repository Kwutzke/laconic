package task

func f() {
	// TODO(#456) fix the parser
	x := 1
	_ = x

	// FIXME(KAT-12) leaks a file handle on the error path
	y := 2
	_ = y
}
