/** Assertion helper returned by `expect()`. */
export interface Expect {
  /** Invert the next assertion. */
  readonly not: Expect;
  /** Strict equality (`===`). */
  toBe(expected: unknown): void;
  /** Deep equality via JSON serialization. */
  toEqual(expected: unknown): void;
  /** Passes when value is not `undefined`. */
  toBeDefined(): void;
  /** Passes when value is `undefined`. */
  toBeUndefined(): void;
  /** Passes when value is `null`. */
  toBeNull(): void;
  /** Passes when value is truthy. */
  toBeTruthy(): void;
  /** Passes when value is falsy. */
  toBeFalsy(): void;
  /** Passes when value > n. */
  toBeGreaterThan(n: number): void;
  /** Passes when value >= n. */
  toBeGreaterThanOrEqual(n: number): void;
  /** Passes when value < n. */
  toBeLessThan(n: number): void;
  /** Passes when value <= n. */
  toBeLessThanOrEqual(n: number): void;
  /** Passes when string or array contains item. */
  toContain(item: unknown): void;
  /** Passes when object has the given property path (dot-separated). Optionally checks value. */
  toHaveProperty(keyPath: string, value?: unknown): void;
  /** Passes when string or array has the given length. */
  toHaveLength(n: number): void;
  /** Passes when value matches a string substring or RegExp. */
  toMatch(pattern: string | RegExp): void;
  /** Passes when the function throws. Optionally matches the error message. */
  toThrow(expected?: string | RegExp): void;
}

/** Register a test case. */
export function test(name: string, fn: () => void | Promise<void>): void;

/** Alias for `test`. */
export function it(name: string, fn: () => void | Promise<void>): void;

/** Group tests under a suite name prefix. */
export function describe(name: string, fn: () => void): void;

/** Wrap a value for assertions. */
export function expect(value: unknown): Expect;
