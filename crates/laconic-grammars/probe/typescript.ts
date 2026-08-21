// a line comment

/* a block comment */

/**/

/****************/

<!-- an html comment -->

/** Documents an exported function. */
export function exported(a: number): number {
  const x = a + 1; // a trailing comment
  console.log(x);

  // a detached comment

  return x;
}

function hidden(): void {}

export interface Shape {}

type Local = number;

export class Thing {
  /** Documents a public member. */
  public visible(): number {
    return 1;
  }

  private concealed(): void {}
}
