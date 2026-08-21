//go:build linux

// Package probe exercises every comment form the Go grammar produces.
package probe

import "fmt"

/* a detached block comment */

// MaxDepth is an exported constant.
const MaxDepth = 3

var defaultName = "probe"

// Config is an exported type.
type Config struct{}

type internalState struct{}

// Exported documents an exported function.
func Exported(a int) error {
	x := a + 1 // a trailing comment
	fmt.Println(x)

	// a detached comment

	return nil
}

// unexported documents an unexported function.
func unexported() {}
