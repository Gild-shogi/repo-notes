import { describe, expect, it } from "vitest";
import { canAutoSave } from "./saveState";

describe("切断後の保存", () => {
  it("結果不明の保存はblurから再送しない", () => {
    expect(canAutoSave({ kind: "indeterminate", message: "connection closed" })).toBe(false);
  });
});
