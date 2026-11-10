import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const script = resolve("scripts/release.ts");

test("release check rejects mismatched app versions before staging a backend", () => {
  const root = mkdtempSync(join(tmpdir(), "repo-notes-release-"));
  try {
    mkdirSync(join(root, "src-tauri", "src"), { recursive: true });
    writeFileSync(join(root, "package.json"), JSON.stringify({ version: "0.1.0" }));
    writeFileSync(join(root, "src-tauri", "tauri.conf.json"), JSON.stringify({ version: "0.1.0" }));
    writeFileSync(join(root, "src-tauri", "Cargo.toml"), '[package]\nversion = "0.2.0"\n');
    writeFileSync(join(root, "src-tauri", "src", "backend.rs"), "pub const PROTOCOL_VERSION: u32 = 1;\n");
    const result = spawnSync(process.execPath, [script, "check", "v0.1.0"], { cwd: root, encoding: "utf8" });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /version mismatch/i);
    writeFileSync(join(root, "src-tauri", "Cargo.toml"), '[package]\nname = "fixture"\nversion = "0.1.0"\n');
    const matched = spawnSync(process.execPath, [script, "check", "v0.1.0"], { cwd: root, encoding: "utf8" });
    assert.equal(matched.status, 0, matched.stderr);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("backend resource verification rejects modified binary content", () => {
  const root = mkdtempSync(join(tmpdir(), "repo-notes-release-"));
  try {
    mkdirSync(join(root, "src-tauri", "src"), { recursive: true });
    mkdirSync(join(root, "backend"));
    writeFileSync(join(root, "package.json"), JSON.stringify({ version: "0.1.0" }));
    writeFileSync(join(root, "src-tauri", "tauri.conf.json"), JSON.stringify({ version: "0.1.0" }));
    writeFileSync(join(root, "src-tauri", "Cargo.toml"), '[package]\nversion = "0.1.0"\n');
    writeFileSync(join(root, "src-tauri", "src", "backend.rs"), "pub const PROTOCOL_VERSION: u32 = 1;\n");
    const bytes = Buffer.alloc(64);
    bytes.write("7f454c460201", "hex");
    bytes.writeUInt16LE(62, 18);
    writeFileSync(join(root, "backend", "repo-notes-backend"), bytes);
    writeFileSync(join(root, "backend", "manifest.json"), JSON.stringify({ version: "0.1.0", protocolVersion: 1, sha256: createHash("sha256").update(bytes).digest("hex") }));
    const verified = spawnSync(process.execPath, [script, "verify-backend", "backend"], { cwd: root, encoding: "utf8" });
    assert.equal(verified.status, 0, verified.stderr);
    bytes[63] = 1;
    writeFileSync(join(root, "backend", "repo-notes-backend"), bytes);
    const modified = spawnSync(process.execPath, [script, "verify-backend", "backend"], { cwd: root, encoding: "utf8" });
    assert.notEqual(modified.status, 0);
    assert.match(modified.stderr, /SHA256 mismatch/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
