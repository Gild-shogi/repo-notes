import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { appendFileSync, chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

type Version = { version: string; protocolVersion: number };
type BackendManifest = Version & { sha256: string };
const backendDir = "src-tauri/resources/backend";
const json = (path: string): unknown => JSON.parse(readFileSync(path, "utf8"));
const sha256 = (path: string): string => createHash("sha256").update(readFileSync(path)).digest("hex");
const NUMBER = "(?:0|[1-9]\\d*)";
const PRERELEASE = `(?:${NUMBER}|\\d*[A-Za-z-][0-9A-Za-z-]*)`;
const VERSION = new RegExp(`^${NUMBER}\\.${NUMBER}\\.${NUMBER}(?:-${PRERELEASE}(?:\\.${PRERELEASE})*)?(?![\\s\\S])`);

function projectVersion(tag?: string): Version {
  const packageVersion = (json("package.json") as { version: string }).version;
  const tauriVersion = (json("src-tauri/tauri.conf.json") as { version: string }).version;
  const cargo = readFileSync("src-tauri/Cargo.toml", "utf8").split(/^\[/m).find((section) => section.startsWith("package]"));
  const cargoVersion = cargo?.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
  if (!VERSION.test(packageVersion) || cargoVersion !== packageVersion || tauriVersion !== packageVersion || (tag !== undefined && tag !== `v${packageVersion}`)) {
    throw new Error("release version mismatch: tag, package.json, Cargo.toml and tauri.conf.json must match");
  }
  const protocol = readFileSync("src-tauri/src/backend.rs", "utf8").match(/^pub const PROTOCOL_VERSION: u32 = (\d+);$/m)?.[1];
  if (protocol === undefined) throw new Error("backend protocol version was not found");
  return { version: packageVersion, protocolVersion: Number(protocol) };
}

function prepareBuild(tag?: string): Version {
  if (tag === undefined) return projectVersion();
  const version = tag.startsWith("v") ? tag.slice(1) : "";
  if (!VERSION.test(version)) throw new Error("invalid release tag: expected vX.Y.Z, for example v1.0.0");
  projectVersion();
  const cargo = readFileSync("src-tauri/Cargo.toml", "utf8");
  const section = cargo.split(/(?=^\[)/m).find((part) => part.startsWith("[package]"));
  const name = section?.match(/^name\s*=\s*"([^"]+)"/m)?.[1];
  if (!section || !name) throw new Error("Cargo package name was not found");
  const stamp = (text: string) => text.replace(/^(version[ \t]*=[ \t]*")[^"]+(")/m, (_match, prefix, suffix) => `${prefix}${version}${suffix}`);
  let matched = 0;
  const lock = readFileSync("src-tauri/Cargo.lock", "utf8").split(/(?=^\[\[package\]\])/m).map((part) => {
    if (part.match(/^name\s*=\s*"([^"]+)"/m)?.[1] !== name) return part;
    if (!/^version[ \t]*=[ \t]*"[^"]+"/m.test(part)) throw new Error("Cargo.lock app version was not found");
    matched += 1;
    return stamp(part);
  }).join("");
  if (matched !== 1) throw new Error("Cargo.lock must contain exactly one app package");
  const packageJson = json("package.json") as { version: string };
  const tauri = json("src-tauri/tauri.conf.json") as { version: string };
  packageJson.version = version;
  tauri.version = version;
  for (const [file, content] of [
    ["package.json", JSON.stringify(packageJson, null, 2) + "\n"],
    ["src-tauri/tauri.conf.json", JSON.stringify(tauri, null, 2) + "\n"],
    ["src-tauri/Cargo.toml", cargo.replace(section, stamp(section))],
    ["src-tauri/Cargo.lock", lock],
  ]) writeFileSync(file, content);
  return projectVersion(tag);
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


type Platform = "macos" | "windows" | "backend";
type BuildReport = Version & {
  platform: Platform;
  file: string;
  sha256: string;
  signed: boolean;
  notarized: boolean;
  backendSha256?: string;
};

function run(command: string, args: string[]): string {
  const result = spawnSync(command, args, { encoding: "utf8", timeout: 30_000 });
  if (result.error || result.status !== 0) throw new Error(`${command} failed: ${result.error?.message || result.stderr.trim() || `exit ${result.status}`}`);
  return result.stdout.trim();
}

function releaseFile(platform: Platform, version: string): string {
  if (platform === "macos") return `repo-notes_${version}_macos_universal.dmg`;
  if (platform === "windows") return `repo-notes_${version}_windows_x64_setup.exe`;
  return `repo-notes-backend_${version}_linux_x64.tar.gz`;
}

function report(output: string, value: BuildReport): void {
  writeFileSync(join(output, `${value.platform}.json`), JSON.stringify(value, null, 2) + "\n");
}

function packageBackend(output: string): void {
  const manifest = verifyBackend(backendDir);
  mkdirSync(output, { recursive: true });
  const file = releaseFile("backend", manifest.version);
  run("tar", ["-czf", resolve(output, file), "-C", backendDir, "repo-notes-backend", "manifest.json", "-C", resolve("."), "LICENSE", "THIRD-PARTY-NOTICES.md"]);
  report(output, { ...projectVersion(), platform: "backend", file, sha256: sha256(join(output, file)), backendSha256: manifest.sha256, signed: false, notarized: false });
}

function windowsSignature(path: string): boolean {
  const command = 'if ((Get-AuthenticodeSignature -LiteralPath $env:REPO_NOTES_SIGN_FILE).Status -eq "Valid") { exit 0 } else { exit 1 }';
  return spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-EncodedCommand", Buffer.from(command, "utf16le").toString("base64")], {
    env: { ...process.env, REPO_NOTES_SIGN_FILE: resolve(path) }, timeout: 30_000, stdio: "ignore",
  }).status === 0;
}

function collect(platform: string, bundle: string, output: string, executable: string): void {
  const expected = projectVersion();
  if (platform !== "macos" && platform !== "windows") throw new Error("unknown installer platform");
  const suffix = platform === "macos" ? ".dmg" : ".exe";
  const matches = readdirSync(bundle, { recursive: true }).filter((entry) => typeof entry === "string" && entry.endsWith(suffix));
  if (matches.length !== 1) throw new Error(`expected exactly one ${suffix} installer`);
  if (platform === "macos") {
    const arches = run("lipo", ["-archs", executable]).split(/\s+/);
    if (!arches.includes("arm64") || !arches.includes("x86_64")) throw new Error("macOS app is not universal");
  } else {
    const pe = readFileSync(executable);
    const offset = pe.length >= 64 ? pe.readUInt32LE(60) : -1;
    if (offset < 0 || offset + 6 > pe.length || pe.readUInt32LE(offset) !== 0x4550 || pe.readUInt16LE(offset + 4) !== 0x8664) throw new Error("Windows app is not x86_64 PE");
  }
  mkdirSync(output, { recursive: true });
  const file = releaseFile(platform, expected.version);
  copyFileSync(join(bundle, matches[0] as string), join(output, file));
  let signed = false;
  let notarized = false;
  let backendSha256: string | undefined;
  if (platform === "macos") {
    const app = resolve(executable, "../../..");
    const verify = spawnSync("codesign", ["--verify", "--deep", "--strict", app], { stdio: "ignore" });
    const details = spawnSync("codesign", ["-dv", "--verbose=4", app], { encoding: "utf8" });
    signed = verify.status === 0 && /Authority=Developer ID Application:/.test(details.stderr ?? "");
    notarized = signed && spawnSync("spctl", ["--assess", "--type", "execute", app], { stdio: "ignore" }).status === 0 && spawnSync("xcrun", ["stapler", "validate", join(output, file)], { stdio: "ignore" }).status === 0;
  } else {
    backendSha256 = verifyBackend(backendDir).sha256;
    signed = windowsSignature(executable) && windowsSignature(join(output, file));
  }
  report(output, { ...expected, platform, file, sha256: sha256(join(output, file)), signed, notarized, ...(backendSha256 ? { backendSha256 } : {}) });
}

function prepareRelease(directory: string, tag: string): void {
  const expected = projectVersion(tag);
  const reports = (["macos", "windows", "backend"] as const).map((platform) => {
    const value = json(join(directory, `${platform}.json`)) as BuildReport;
    if (value.platform !== platform || value.version !== expected.version || value.protocolVersion !== expected.protocolVersion || value.file !== releaseFile(platform, expected.version) || value.sha256 !== sha256(join(directory, value.file))) throw new Error(`invalid ${platform} release artifact`);
    return value;
  });
  const [macos, windows, backend] = reports;
  if (!/^[a-f0-9]{64}$/.test(backend.backendSha256 ?? "") || windows.backendSha256 !== backend.backendSha256) throw new Error("Windows installer backend differs from release backend");
  const publishable = macos.signed === true && macos.notarized === true && windows.signed === true;
  const destination = join(directory, "prepared");
  mkdirSync(destination);
  for (const item of reports) copyFileSync(join(directory, item.file), join(destination, item.file));
  writeFileSync(join(destination, "release-manifest.json"), JSON.stringify({ ...expected, publishable, artifacts: reports }, null, 2) + "\n");
  const files = [...reports.map((item) => item.file), "release-manifest.json"];
  writeFileSync(join(destination, "SHA256SUMS"), files.map((file) => `${sha256(join(destination, file))}  ${file}`).join("\n") + "\n");
  writeFileSync(join(directory, "release-notes.md"), `repo-notes ${expected.version}\n\nValidation targets: macOS 13+ (Intel / Apple Silicon), Windows 11 x64 + Ubuntu WSL2.\n\n${publishable ? "Platform signature and notarization checks passed." : "Validation build: signing is incomplete. This release must remain Draft."}\n`);
  outputs({ ...expected, tag, publishable });
}

function publishRelease(directory: string, tag: string): void {
  if (process.env.GITHUB_ACTIONS !== "true" || process.env.GITHUB_EVENT_NAME !== "push" || process.env.GITHUB_REF_TYPE !== "tag" || process.env.GITHUB_REF_NAME !== tag) throw new Error("release writes require a tag push workflow");
  const expected = projectVersion(tag);
  const prepared = join(directory, "prepared");
  const manifest = json(join(prepared, "release-manifest.json")) as Version & { publishable: boolean; artifacts: BuildReport[] };
  if (manifest.version !== expected.version || manifest.protocolVersion !== expected.protocolVersion) throw new Error("prepared release version mismatch");
  const files = [...manifest.artifacts.map((item) => join(prepared, item.file)), join(prepared, "release-manifest.json"), join(prepared, "SHA256SUMS")];
  const releases = JSON.parse(run("gh", ["release", "list", "--limit", "100", "--json", "tagName,isDraft"])) as { tagName: string; isDraft: boolean }[];
  const existing = releases.find((release) => release.tagName === tag);
  if (existing && !existing.isDraft) throw new Error("published releases are immutable; refusing to replace assets");
  if (existing) run("gh", ["release", "upload", tag, ...files, "--clobber"]);
  else run("gh", ["release", "create", tag, ...files, "--verify-tag", "--draft", "--title", `repo-notes ${expected.version}`, "--notes-file", join(directory, "release-notes.md")]);
  if (manifest.publishable === true) run("gh", ["release", "edit", tag, "--draft=false"]);
}

function outputs(values: Record<string, string | number | boolean>) {
  for (const [key, value] of Object.entries(values)) {
    console.log(`${key}=${value}`);
    if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `${key}=${value}\n`);
  }
}

try {
  const [command, argument, second, third, fourth] = process.argv.slice(2);
  if (command === "prepare-build" && argument === "--help") console.log("Usage: node scripts/release.ts prepare-build [vX.Y.Z]\n\nTag builds write the tag version to package.json, Tauri, Cargo.toml and Cargo.lock. Other builds only check the repository version. The tag defaults to GITHUB_REF_NAME when GITHUB_REF_TYPE=tag.\n\nExamples:\n  node scripts/release.ts prepare-build v1.0.0\n  node scripts/release.ts prepare-build");
  else if (command === "prepare-build") outputs(prepareBuild(argument ?? (process.env.GITHUB_REF_TYPE === "tag" ? process.env.GITHUB_REF_NAME : undefined)));
  else if (command === "check") outputs(projectVersion(argument ?? (process.env.GITHUB_REF_TYPE === "tag" ? process.env.GITHUB_REF_NAME : undefined)));
  else if (command === "stage-backend" && argument) outputs(stageBackend(argument));
  else if (command === "verify-backend") outputs(verifyBackend(argument ?? backendDir));
  else if (command === "package-backend" && argument) packageBackend(argument);
  else if (command === "collect" && argument && second && third && fourth) collect(argument, second, third, fourth);
  else if (command === "prepare-release" && argument && second) prepareRelease(argument, second);
  else if (command === "publish-release" && argument && second) publishRelease(argument, second);
  else throw new Error(`unknown release command: ${command ?? "(missing)"}`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
