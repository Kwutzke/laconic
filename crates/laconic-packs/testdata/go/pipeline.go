//go:build linux

// Copyright 2026 nobody. All rights reserved.

// Package pipeline exercises every stage the engine runs.
package pipeline

import "fmt"

/* a detached block comment */

// Exported documents an exported function.
func Exported(a int) error {
	x := a + 1 // a trailing comment
	fmt.Println(x)

	// a detached comment

	// laconic:ignore narration — the line below is load-bearing
	// this comment is protected
	return nil
}

// unexported does nothing.
func unexported() {}

// a first narration line
// a second narration line
func multiline() {}

// MaxDepth is an exported constant.
const MaxDepth = 3

type internalState struct{}
