import { describe, test, expect } from "husako/test";
import husako from "husako";

describe("husako.build()", () => {
  test("throws TypeError for non-builder input", () => {
    expect(() => husako.build([{ not: "a builder" }])).toThrow("build(): item at index 0");
  });
  test("throws TypeError for plain object (non-array)", () => {
    expect(() => husako.build({ not: "a builder" } as any)).toThrow("build(): item at index 0");
  });
  test("throws TypeError for null item", () => {
    expect(() => husako.build([null as any])).toThrow("build(): item at index 0");
  });
});
