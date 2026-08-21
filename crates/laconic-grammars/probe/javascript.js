// a line comment

/* a block comment */

<!-- an html comment -->

/** Documents an exported function. */
export function exported(a) {
  const x = a + 1; // a trailing comment
  console.log(x);

  // a detached comment

  return x;
}

function hidden() {}
