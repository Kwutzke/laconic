// a line comment

/* a block comment */

<!-- an html comment -->

/** Documents an exported component. */
export function Component(): JSX.Element {
  const x = 1;
  console.log(x);
  return <div className="x">text</div>;
}

function hidden(): void {}
