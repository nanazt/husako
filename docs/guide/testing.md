# Writing Tests

husako includes a built-in test runner for TypeScript files. You can write unit tests for your
resource factory helpers, value transformers, and other reusable logic, then run them with
`husako test`.

## When to write husako tests

husako tests are useful for:

- Unit-testing resource factory helpers before using them in render files
- Verifying that computed values (labels, specs, resource quantities) are produced correctly
- Regression tests for shared library code in multi-team projects
- Testing plugin helper functions

## Setup

Before running tests, you need the `husako/test` module on the TypeScript path. Run `husako gen`
once (or `husako gen --skip-k8s` if you do not need k8s types):

```bash
husako gen --skip-k8s
```

This writes `.husako/types/husako/test.d.ts` and adds `"husako/test"` to `tsconfig.json`.

## Writing a test file

Test files must be named `*.test.ts` or `*.spec.ts`. Place them anywhere in your project.

```typescript
// helpers.test.ts
import { test, describe, expect } from "husako/test";
import { makeDeployment } from "./helpers";

describe("Deployment factory", () => {
  test("sets replicas", () => {
    const doc = makeDeployment("my-app", 3)._render();
    expect(doc.spec.replicas).toBe(3);
  });

  test("sets name", () => {
    const doc = makeDeployment("my-app", 3)._render();
    expect(doc.metadata.name).toBe("my-app");
  });
});
```

## Running tests

```bash
# Run all discovered test files
husako test

# Run a specific file
husako test helpers.test.ts

# Run multiple files
husako test helpers.test.ts utils.test.ts
```

husako discovers `*.test.ts` and `*.spec.ts` recursively from the project root, skipping
`.husako/`, `node_modules/`, and hidden directories.

## API reference

### `test(name, fn)`

Register a test case. `fn` can be synchronous or async.

```typescript
test("plain arithmetic", () => {
  expect(1 + 1).toBe(2);
});

test("async logic", async () => {
  const result = await Promise.resolve(42);
  expect(result).toBe(42);
});
```

### `it(name, fn)`

Alias for `test`.

### `describe(name, fn)`

Group tests under a suite name. Nested `describe` blocks are separated by ` > ` in test names.

```typescript
describe("metadata helpers", () => {
  test("name", () => { /* ... */ });       // "metadata helpers > name"
  describe("labels", () => {
    test("add", () => { /* ... */ });     // "metadata helpers > labels > add"
  });
});
```

### `expect(value)`

Returns an `Expect` object for making assertions. Use `.not` to invert any assertion:

```typescript
expect(result).not.toBe(undefined);
```

### Expect methods

| Method | Passes when |
|--------|-------------|
| `.toBe(expected)` | `value === expected` (strict equality) |
| `.toEqual(expected)` | Deep equality via `JSON.stringify` |
| `.toBeDefined()` | `value !== undefined` |
| `.toBeUndefined()` | `value === undefined` |
| `.toBeNull()` | `value === null` |
| `.toBeTruthy()` | `!!value` is true |
| `.toBeFalsy()` | `!!value` is false |
| `.toBeGreaterThan(n)` | `value > n` |
| `.toBeGreaterThanOrEqual(n)` | `value >= n` |
| `.toBeLessThan(n)` | `value < n` |
| `.toBeLessThanOrEqual(n)` | `value <= n` |
| `.toContain(item)` | String includes substring, or array includes item |
| `.toHaveProperty(path, value?)` | Object has property at dot-path (optionally matching value) |
| `.toHaveLength(n)` | `value.length === n` |
| `.toMatch(pattern)` | String matches substring or `RegExp` |
| `.toThrow(pattern?)` | Function throws (optionally matching error message) |

## Testing with k8s builders

Call `._render()` on a builder to get the plain JSON object, then assert on its fields:

```typescript
import { test, expect } from "husako/test";
import { Deployment } from "k8s/apps/v1";
import { name, label } from "husako";

test("Deployment has correct metadata", () => {
  const doc = new Deployment()
    .metadata(name("web").label("app", "web"))
    ._render();

  expect(doc.apiVersion).toBe("apps/v1");
  expect(doc.kind).toBe("Deployment");
  expect(doc.metadata.name).toBe("web");
  expect(doc.metadata.labels.app).toBe("web");
});
```

Note: k8s builders require `husako gen` (not just `--skip-k8s`) to be available.

## Plugin testing

If your project uses plugins, run `husako gen` first to install them, then import from
the plugin specifier as usual:

```typescript
// plugin.test.ts
import { test, expect } from "husako/test";
import { greet } from "myplugin";

test("greet returns expected string", () => {
  expect(greet("World")).toBe("Hello, World!");
});
```

Run:

```bash
husako gen --skip-k8s   # installs plugins, writes types
husako test plugin.test.ts
```

## Exit codes

| Exit code | Meaning |
|-----------|---------|
| 0 | All tests passed |
| 1 | One or more tests failed, or a file could not be compiled/run |
