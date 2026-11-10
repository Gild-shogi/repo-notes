import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { appendFileSync, chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

type Version = { version: string; protocolVersion: number };
type BackendManifest = Version & { sha256: string };
const backendDir = "src-tauri/resources/backend";
const json = (path: string): unknown => JSON.parse(readFileSync(path, "utf8"));
const sha256 = (path: string): string => createHash("sha256").update(readFileSync(path)).digest("hex");

function projectVersion(tag?: string): Version {
  const packageVersion = (json("package.json") as { version: string }).version;
  const tauriVersion = (json("src-tauri/tauri.conf.json") as { version: string }).version;
  const cargo = readFileSync("src-tauri/Cargo.toml", "utf8").split(/^\[/m).find((section) => section.startsWith("package]"));
  const cargoVersion = cargo?.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?$/.test(packageVersion) || cargoVersion !== packageVersion || tauriVersion !== packageVersion || (tag !== undefined && tag !== `v${packageVersion}`)) {
    throw new Error("release version mismatch: tag, package.json, Cargo.toml and tauri.conf.json must match");
  }
  const protocol = readFileSync("src-tauri/src/backend.rs", "utf8").match(/^pub const PROTOCOL_VERSION: u32 = (\d+);$/m)?.[1];
  if (protocol === undefined) throw new Error("backend protocol version was not found");
  return { version: packageVersion, protocolVersion: Number(protocol) };
}

function verifyBackend(directory: string): BackendManifest {
  const expected = projectVersion();
  const manifest = json(join(directory, "manifest.json")) as BackendManifest;
  const binary = join(directory, "repo-notes-backend");
  const bytes = readFileSync(binary);
  if (manifest.version !== expected.version || manifest.protocolVersion !== expected.protocolVersion || !/^[a-f0-9]{64}$/.test(manifest.sha256) || sha256(binary) !== manifest.sha256) {
    throw new Error("backend version or SHA256 mismatch");
  }
  if (bytes.length < 20 || bytes.subarray(0, 4).toString("hex") !== "7f454c46" || bytes[4] !== 2 || bytes[5] !== 1 || bytes.readUInt16LE(18) !== 62) {
    throw new Error("backend must be a Linux x86_64 ELF binary");
  }
  return manifest;
}

function stageBackend(source: string): BackendManifest {
  const expected = projectVersion();
  const version = spawnSync(resolve(source), ["--version"], { encoding: "utf8", timeout: 10_000 });
  if (version.error || version.status !== 0 || version.stdout.trim() !== expected.version) throw new Error("backend executable version mismatch");
  const fixture = mkdtempSync(join(tmpdir(), "repo-notes-release-"));
  try {
    const input = [
      { id: 1, method: "hello", appVersion: expected.version, protocolVersion: expected.protocolVersion, dataRoot: join(fixture, "notes") },
      { id: 2, method: "shutdown" },
    ].map((request) => JSON.stringify(request)).join("\n") + "\n";
    const connected = spawnSync(resolve(source), ["--stdio"], { input, encoding: "utf8", timeout: 10_000 });
    const hello = connected.stdout.split("\n").filter(Boolean).map((line) => JSON.parse(line)).find((message) => message.type === "response" && message.id === 1)?.result;
    if (connected.error || connected.status !== 0 || hello?.appVersion !== expected.version || hello?.protocolVersion !== expected.protocolVersion) throw new Error("backend handshake version mismatch");
  } finally {
    rmSync(fixture, { recursive: true, force: true });
  }
  mkdirSync(backendDir, { recursive: true });
  const binary = join(backendDir, "repo-notes-backend");
  copyFileSync(source, binary);
  chmodSync(binary, 0o755);
  const manifest = { ...expected, sha256: sha256(binary) };
  writeFileSync(join(backendDir, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  return verifyBackend(backendDir);
}

function outputs(values: Record<string, string | number | boolean>) {
  for (const [key, value] of Object.entries(values)) {
    console.log(`${key}=${value}`);
    if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `${key}=${value}\n`);
  }
}

try {
  const [command, argument] = process.argv.slice(2);
  if (command === "check") outputs(projectVersion(argument));
  else if (command === "stage-backend" && argument) outputs(stageBackend(argument));
  else if (command === "verify-backend") outputs(verifyBackend(argument ?? backendDir));
  else throw new Error(`unknown release command: ${command ?? "(missing)"}`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
