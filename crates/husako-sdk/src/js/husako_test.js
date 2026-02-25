// "husako/test" module: Jest-like test runner for husako TypeScript files
// Evaluated by QuickJS; globalThis.__husako_run_all_tests is called by Rust.

const _tests = [];
const _suiteStack = [];

export function describe(name, fn) {
  _suiteStack.push(name);
  fn();
  _suiteStack.pop();
}

function _qualify(name) {
  if (_suiteStack.length === 0) return name;
  return _suiteStack.join(" > ") + " > " + name;
}

export function test(name, fn) {
  _tests.push({ name: _qualify(name), fn });
}

export const it = test;

class AssertionError extends Error {
  constructor(message) {
    super(message);
    this.name = "AssertionError";
  }
}

class Expect {
  constructor(value, negated) {
    this._value = value;
    this._negated = negated || false;
  }

  get not() {
    return new Expect(this._value, !this._negated);
  }

  _assert(condition, message) {
    const passes = this._negated ? !condition : condition;
    if (!passes) {
      if (this._negated) {
        throw new AssertionError("Expected NOT: " + message);
      } else {
        throw new AssertionError(message);
      }
    }
  }

  toBe(expected) {
    this._assert(
      this._value === expected,
      "Expected " + JSON.stringify(this._value) + " to be " + JSON.stringify(expected)
    );
  }

  toEqual(expected) {
    this._assert(
      JSON.stringify(this._value) === JSON.stringify(expected),
      "Expected " + JSON.stringify(this._value) + " to equal " + JSON.stringify(expected)
    );
  }

  toBeDefined() {
    this._assert(
      this._value !== undefined,
      "Expected value to be defined, but got undefined"
    );
  }

  toBeUndefined() {
    this._assert(
      this._value === undefined,
      "Expected " + JSON.stringify(this._value) + " to be undefined"
    );
  }

  toBeNull() {
    this._assert(
      this._value === null,
      "Expected " + JSON.stringify(this._value) + " to be null"
    );
  }

  toBeTruthy() {
    this._assert(
      !!this._value,
      "Expected " + JSON.stringify(this._value) + " to be truthy"
    );
  }

  toBeFalsy() {
    this._assert(
      !this._value,
      "Expected " + JSON.stringify(this._value) + " to be falsy"
    );
  }

  toBeGreaterThan(n) {
    this._assert(
      this._value > n,
      "Expected " + this._value + " to be greater than " + n
    );
  }

  toBeGreaterThanOrEqual(n) {
    this._assert(
      this._value >= n,
      "Expected " + this._value + " to be >= " + n
    );
  }

  toBeLessThan(n) {
    this._assert(
      this._value < n,
      "Expected " + this._value + " to be less than " + n
    );
  }

  toBeLessThanOrEqual(n) {
    this._assert(
      this._value <= n,
      "Expected " + this._value + " to be <= " + n
    );
  }

  toContain(item) {
    if (typeof this._value === "string") {
      this._assert(
        this._value.includes(item),
        "Expected string " + JSON.stringify(this._value) + " to contain " + JSON.stringify(item)
      );
    } else if (Array.isArray(this._value)) {
      this._assert(
        this._value.includes(item),
        "Expected array to contain " + JSON.stringify(item)
      );
    } else {
      throw new AssertionError("toContain requires a string or array, got " + typeof this._value);
    }
  }

  toHaveProperty(keyPath, value) {
    const keys = typeof keyPath === "string" ? keyPath.split(".") : [keyPath];
    let obj = this._value;
    for (const k of keys) {
      if (obj === null || obj === undefined || typeof obj !== "object") {
        this._assert(false, "Expected object to have property " + JSON.stringify(keyPath));
        return;
      }
      obj = obj[k];
    }
    if (arguments.length >= 2) {
      this._assert(
        JSON.stringify(obj) === JSON.stringify(value),
        "Expected property " + JSON.stringify(keyPath) + " to equal " + JSON.stringify(value) + ", got " + JSON.stringify(obj)
      );
    } else {
      this._assert(
        obj !== undefined,
        "Expected object to have property " + JSON.stringify(keyPath)
      );
    }
  }

  toHaveLength(n) {
    const len = this._value == null ? undefined : this._value.length;
    this._assert(
      len === n,
      "Expected length " + len + " to equal " + n
    );
  }

  toMatch(pattern) {
    if (typeof pattern === "string") {
      this._assert(
        this._value.includes(pattern),
        "Expected " + JSON.stringify(this._value) + " to match " + JSON.stringify(pattern)
      );
    } else {
      this._assert(
        pattern.test(this._value),
        "Expected " + JSON.stringify(this._value) + " to match " + pattern
      );
    }
  }

  toThrow(expected) {
    let threw = false;
    let thrownMsg = "";
    try {
      this._value();
    } catch (e) {
      threw = true;
      thrownMsg = e instanceof Error ? e.message : String(e);
    }

    if (arguments.length === 0) {
      this._assert(threw, "Expected function to throw");
    } else if (typeof expected === "string") {
      this._assert(threw && thrownMsg.includes(expected),
        "Expected function to throw containing " + JSON.stringify(expected) + ", got " + JSON.stringify(thrownMsg));
    } else if (expected instanceof RegExp) {
      this._assert(threw && expected.test(thrownMsg),
        "Expected function to throw matching " + expected + ", got " + JSON.stringify(thrownMsg));
    } else {
      this._assert(threw, "Expected function to throw");
    }
  }
}

export function expect(value) {
  return new Expect(value);
}

// Called by Rust after module evaluation
globalThis.__husako_run_all_tests = async function () {
  const results = [];
  for (const { name, fn } of _tests) {
    try {
      await fn();
      results.push({ name, passed: true, error: null });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      results.push({ name, passed: false, error: msg });
    }
  }
  return JSON.stringify(results);
};
