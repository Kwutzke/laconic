/// <reference path="./globals.d.ts" />
//# sourceMappingURL=out.js.map
// @ts-nocheck
// @ts-expect-error
// eslint-disable-next-line no-console
// prettier-ignore
// istanbul ignore next
// c8 ignore next
// biome-ignore lint: intentional
// @formatter:off
// v8 ignore next
// webpackChunkName: "chunk"
// @jsx createElement
// @license MIT
// @preserve keep this banner

/** Documents an exported function. */
export function exported(a: number): number {
  const x = a + 1;
  return x;
}

function hidden(): void {}

export interface Shape {}

type Local = number;

const _internal = 3;

export class Thing {
  /** Documents a public member. */
  public visible(): number {
    return 1;
  }

  private concealed(): void {}
}
