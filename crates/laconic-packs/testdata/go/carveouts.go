//go:build linux
// +build linux

// Package carveouts holds one line per directive the Go pack strips.
package carveouts

//nolint:gosec
//lint:ignore SA1000 the pattern is a constant
//export CallMe
// #cgo CFLAGS: -I/usr/local/include

func exported() {}
