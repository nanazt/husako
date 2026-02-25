import { describe, test, expect } from "husako/test";

describe("toBe", () => {
  test("equal primitives pass", () => { expect(1).toBe(1); });
  test(".not.toBe different values", () => { expect(1).not.toBe(2); });
});
describe("toEqual", () => {
  test("deep equal objects", () => { expect({ a: 1 }).toEqual({ a: 1 }); });
  test(".not.toEqual different", () => { expect({ a: 1 }).not.toEqual({ a: 2 }); });
});
describe("toBeDefined / toBeUndefined", () => {
  test("defined value", () => { expect("x").toBeDefined(); });
  test("undefined value", () => { expect(undefined).toBeUndefined(); });
  test(".not.toBeDefined", () => { expect(undefined).not.toBeDefined(); });
  test(".not.toBeUndefined", () => { expect("x").not.toBeUndefined(); });
});
describe("toBeNull", () => {
  test("null value", () => { expect(null).toBeNull(); });
  test(".not.toBeNull", () => { expect(0).not.toBeNull(); });
});
describe("toBeTruthy / toBeFalsy", () => {
  test("truthy", () => { expect(1).toBeTruthy(); });
  test("falsy", () => { expect(0).toBeFalsy(); });
  test(".not.toBeTruthy", () => { expect(0).not.toBeTruthy(); });
  test(".not.toBeFalsy", () => { expect(1).not.toBeFalsy(); });
});
describe("numeric comparisons", () => {
  test("toBeGreaterThan", () => { expect(5).toBeGreaterThan(3); });
  test("toBeGreaterThanOrEqual", () => { expect(5).toBeGreaterThanOrEqual(5); });
  test("toBeLessThan", () => { expect(3).toBeLessThan(5); });
  test("toBeLessThanOrEqual", () => { expect(5).toBeLessThanOrEqual(5); });
  test(".not.toBeGreaterThan", () => { expect(3).not.toBeGreaterThan(5); });
});
describe("toContain", () => {
  test("array contains item", () => { expect([1, 2, 3]).toContain(2); });
  test("string contains substring", () => { expect("hello world").toContain("world"); });
  test(".not.toContain", () => { expect([1, 2]).not.toContain(3); });
});
describe("toHaveLength", () => {
  test("array length", () => { expect([1, 2, 3]).toHaveLength(3); });
  test("string length", () => { expect("abc").toHaveLength(3); });
  test(".not.toHaveLength", () => { expect([1]).not.toHaveLength(3); });
});
describe("toHaveProperty", () => {
  test("property exists", () => { expect({ a: 1 }).toHaveProperty("a"); });
  test("nested property", () => { expect({ a: { b: 2 } }).toHaveProperty("a.b"); });
  test("property with value", () => { expect({ x: 42 }).toHaveProperty("x", 42); });
  test(".not.toHaveProperty", () => { expect({ a: 1 }).not.toHaveProperty("b"); });
});
describe("toMatch", () => {
  test("string contains", () => { expect("hello world").toMatch("world"); });
  test("regexp match", () => { expect("husako-v1.0").toMatch(/v\d+/); });
  test(".not.toMatch", () => { expect("hello").not.toMatch("goodbye"); });
});
describe("toThrow", () => {
  test("function throws", () => { expect(() => { throw new Error("boom"); }).toThrow(); });
  test("function throws with message", () => { expect(() => { throw new Error("boom"); }).toThrow("boom"); });
  test("function throws matching regexp", () => { expect(() => { throw new Error("error 42"); }).toThrow(/\d+/); });
  test(".not.toThrow", () => { expect(() => {}).not.toThrow(); });
});
describe("nested describe", () => {
  describe("inner suite", () => {
    test("nested test runs correctly", () => { expect(true).toBeTruthy(); });
  });
});
