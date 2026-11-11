import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

type LicenseFile = { source: string; text: string };
type Package = { ecosystem: string; name: string; version: string; license: string; source: string; files: LicenseFile[] };
type Manifest = { name: string; version: string; license?: string; dependencies?: Record<string, string>; optionalDependencies?: Record<string, string>; devDependencies?: Record<string, string>; peerDependencies?: Record<string, string>; peerDependenciesMeta?: Record<string, { optional?: boolean }> };
type Crate = { name: string; version: string; license: string | null; source: string | null; manifest_path: string };

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const readJson = (path: string) => JSON.parse(readFileSync(path, 'utf8'));
const supplements: { packages: Record<string, string[]>; files: Record<string, string> } = readJson(join(root, 'scripts/license-supplements.json'));
const packages: Package[] = [];

function licenseFiles(directory: string, source: string): LicenseFile[] {
  const files: LicenseFile[] = [];
  function visit(current: string) {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = join(current, entry.name);
      if (entry.isDirectory() && !['node_modules', '.git', 'target'].includes(entry.name)) visit(path);
      if (entry.isFile() && /^(licen[sc]e|copying|notice|copyright|authors$)|\.LICENSE\./i.test(entry.name)) {
        files.push({ source: `${source} — ${relative(directory, path).split('\\').join('/')}`, text: readFileSync(path, 'utf8') });
      }
    }
  }
  visit(directory);
  if (files.length === 0) {
    for (const entry of readdirSync(directory)) {
      if (!/^readme/i.test(entry)) continue;
      const text = readFileSync(join(directory, entry), 'utf8');
      const start = text.search(/^(?:#{1,6} )?(?:License|Copyright)/im);
      if (start >= 0 && /Permission is hereby granted|Redistribution and use in source and binary forms/.test(text.slice(start))) {
        files.push({ source: `${source} — ${entry}`, text: text.slice(start) });
      }
    }
  }
  return files.sort((a, b) => a.source.localeCompare(b.source, 'en'));
}

function resolvePackage(from: string, name: string): string | undefined {
  for (let current = from; ; current = dirname(current)) {
    const path = join(current, 'node_modules', name, 'package.json');
    if (existsSync(path)) return dirname(realpathSync(path));
    if (dirname(current) === current) return;
  }
}

const visited = new Set<string>();
function visitPackage(directory: string) {
  if (visited.has(directory)) return;
  visited.add(directory);
  const manifest: Manifest = readJson(join(directory, 'package.json'));
  const source = `https://registry.npmjs.org/${manifest.name}/-/${manifest.name.split('/').at(-1)}-${manifest.version}.tgz`;
  const files = licenseFiles(directory, source);
  for (const url of supplements.packages[`${manifest.name}@${manifest.version}`] ?? []) {
    if (!supplements.files[url]) throw new Error(`Missing supplement: ${url}`);
    files.push({ source: url, text: supplements.files[url] });
  }
  if (manifest.name === 'natural-compare' && manifest.version === '1.4.0') {
    const attribution = readFileSync(join(directory, 'README.md'), 'utf8').match(/^Copyright.*$/im)?.[0];
    if (!attribution) throw new Error('Missing natural-compare attribution');
    files.push({ source: `${source} — README.md`, text: attribution });
  }
  if (manifest.name === 'elkjs') {
    const bundled = readFileSync(join(directory, 'lib/elk.bundled.js'), 'utf8');
    for (const [text] of bundled.matchAll(/\/\*[\s\S]*?\*\//g)) {
      if (/copyright/i.test(text)) files.push({ source: `${source} — lib/elk.bundled.js`, text });
    }
  }
  const license = manifest.license ?? (manifest.name === 'khroma' && manifest.version === '2.1.0' && files.some(file => file.text.includes('MIT License')) ? 'MIT' : 'UNKNOWN');
  packages.push({ ecosystem: 'npm', name: manifest.name, version: manifest.version, license, source, files });
  visitDependencies(directory, manifest);
}

function visitDependencies(directory: string, manifest: Manifest, includeDev = false) {
  const required = { ...manifest.peerDependencies, ...manifest.dependencies, ...(includeDev ? manifest.devDependencies : {}) };
  for (const name of Object.keys({ ...required, ...manifest.optionalDependencies }).sort()) {
    const dependency = resolvePackage(directory, name);
    if (dependency) visitPackage(dependency);
    else if (!(name in (manifest.optionalDependencies ?? {})) && !manifest.peerDependenciesMeta?.[name]?.optional) throw new Error(`Missing dependency: ${manifest.name} -> ${name}. Run pnpm install --frozen-lockfile.`);
  }
}
visitDependencies(root, readJson(join(root, 'package.json')), true);

const metadata: { packages: Crate[] } = JSON.parse(execFileSync('cargo', ['metadata', '--manifest-path', join(root, 'src-tauri/Cargo.toml'), '--locked', '--format-version', '1'], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 }));
for (const crate of metadata.packages.filter(crate => crate.source)) {
  if (!crate.source?.startsWith('registry+')) throw new Error(`Review non-registry source: ${crate.name}`);
  const source = `https://static.crates.io/crates/${crate.name}/${crate.name}-${crate.version}.crate`;
  const files = licenseFiles(dirname(crate.manifest_path), source);
  for (const url of supplements.packages[`${crate.name}@${crate.version}`] ?? []) {
    if (!supplements.files[url]) throw new Error(`Missing supplement: ${url}`);
    files.push({ source: url, text: supplements.files[url] });
  }
  if (crate.name === 'selectors' && crate.version === '0.36.1') {
    const cssparser = metadata.packages.find(crate => crate.name === 'cssparser');
    if (!cssparser) throw new Error('Missing MPL text for selectors');
    files.push({ source: `${source} — lib.rs`, text: readFileSync(join(dirname(crate.manifest_path), 'lib.rs'), 'utf8').split('*/')[0] + '*/' });
    files.push(...licenseFiles(dirname(cssparser.manifest_path), `https://static.crates.io/crates/cssparser/cssparser-${cssparser.version}.crate`));
  }
  packages.push({ ecosystem: 'Cargo', name: crate.name, version: crate.version, license: crate.license ?? 'UNKNOWN', source, files });
}

const unique = [...new Map(packages.map(pkg => [`${pkg.ecosystem}:${pkg.name}@${pkg.version}`, pkg])).values()]
  .sort((a, b) => `${a.ecosystem}:${a.name}@${a.version}`.localeCompare(`${b.ecosystem}:${b.name}@${b.version}`, 'en'));
for (const pkg of unique.filter(pkg => pkg.ecosystem === 'npm' && pkg.files.length === 0)) {
  const parent = [
    ['@rolldown/binding-', 'rolldown'],
    ['@oxlint/binding-', 'oxlint'],
    ['@tauri-apps/cli-', '@tauri-apps/cli'],
  ].find(([prefix]) => pkg.name.startsWith(prefix))?.[1];
  if (parent) pkg.files.push(...(unique.find(candidate => candidate.ecosystem === 'npm' && candidate.name === parent && candidate.version === pkg.version)?.files ?? []));
  if (pkg.name === '@humanfs/types') pkg.files.push(...(unique.find(candidate => candidate.ecosystem === 'npm' && candidate.name === '@humanfs/core')?.files ?? []));
}
const missing = unique.filter(pkg => pkg.license === 'UNKNOWN' || pkg.files.length === 0);
if (missing.length) throw new Error(`Review missing licenses: ${missing.map(pkg => `${pkg.ecosystem}:${pkg.name}@${pkg.version}`).join(', ')}`);
if (unique.some(pkg => pkg.name === 'elkjs' && pkg.version !== '0.12.1')) throw new Error('Review elkjs source availability URLs for the new version.');

const texts = new Map<string, { text: string; sources: Set<string> }>();
const inventory = unique.map(pkg => {
  const ids = pkg.files.map(file => {
    const normalized = file.text.replace(/\r\n/g, '\n').trim();
    const id = createHash('sha256').update(normalized).digest('hex').slice(0, 16);
    if (!texts.has(id)) texts.set(id, { text: normalized, sources: new Set() });
    texts.get(id)!.sources.add(file.source);
    return id;
  });
  const selected = pkg.name === 'elkjs' ? 'EPL-2.0' : pkg.name === 'dompurify' ? 'Apache-2.0' : pkg.license;
  return `| ${pkg.ecosystem} | ${pkg.name} | ${pkg.version} | ${selected} | [Source](${pkg.source}) | ${[...new Set(ids)].map(id => `[${id}](#license-${id})`).join(', ')} |`;
});
const output = `# Third-party notices

Generated by scripts/generate-licenses.ts. Do not edit by hand.
repo-notes is MIT-licensed; each dependency retains its own license.
This inventory includes installed npm production/development dependencies and Cargo dependencies for all targets, including build/development dependencies. It is a conservative superset of the application bundle, not a list of code proven to be shipped.

## Source availability

Unmodified MPL-2.0 components remain available under MPL-2.0. Their exact source archives are linked in the inventory. Source files in those archives retain their notices. MPL-2.0: https://www.mozilla.org/MPL/2.0/

elkjs is used under EPL-2.0, not its alternative GPL license. Its corresponding source and build instructions are available at https://github.com/kieler/elkjs/tree/0.12.1 and the ELK source at https://github.com/eclipse/elk/tree/v0.12.1 . EPL-2.0: https://www.eclipse.org/legal/epl-2.0/

DOMPurify is used under its Apache-2.0 option. Other OR or slash-separated license expressions describe alternatives offered by the upstream author; AND expressions require both terms. Including upstream license texts does not relicense repo-notes or select a GPL alternative.

## Inventory

| Ecosystem | Package | Version | License | Source archive | Notices |
| --- | --- | --- | --- | --- | --- |
${inventory.join('\n')}

## License and copyright texts

${[...texts.entries()].sort(([a], [b]) => a.localeCompare(b, 'en')).map(([id, entry]) => `<a id="license-${id}"></a>\n\n### ${id}\n\n${[...entry.sources].sort().map(source => `- ${source}`).join('\n')}\n\n\`\`\`text\n${entry.text}\n\`\`\`\n`).join('\n')}`;
const destination = join(root, 'THIRD-PARTY-NOTICES.md');
if (process.argv.includes('--check')) {
  if (!existsSync(destination) || readFileSync(destination, 'utf8') !== output) throw new Error('Third-party notices are stale. Run pnpm licenses:generate.');
} else {
  writeFileSync(destination, output);
}
console.log(`${unique.filter(pkg => pkg.ecosystem === 'npm').length} npm / ${unique.filter(pkg => pkg.ecosystem === 'Cargo').length} Cargo packages; ${texts.size} notice texts`);
