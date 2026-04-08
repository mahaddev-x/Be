#!/usr/bin/env node
/**
 * BeeHive release script — mirrors pi-mono's release.mjs pattern.
 *
 * Usage:
 *   node scripts/release.mjs patch    # 2.0.0 → 2.0.1
 *   node scripts/release.mjs minor    # 2.0.0 → 2.1.0
 *   node scripts/release.mjs major    # 2.0.0 → 3.0.0
 *
 * What it does:
 *   1. Checks the working directory is clean
 *   2. Bumps the version in package.json
 *   3. Commits + tags vX.Y.Z
 *   4. Pushes to origin — this triggers GitHub Actions to build binaries and publish npm
 */

import { execSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const bump = process.argv[2];
if (!["patch", "minor", "major"].includes(bump)) {
  console.error("Usage: node scripts/release.mjs patch|minor|major");
  process.exit(1);
}

function run(cmd, opts = {}) {
  console.log(`  $ ${cmd}`);
  return execSync(cmd, { encoding: "utf8", stdio: opts.silent ? "pipe" : "inherit", ...opts });
}

// 1. Check working directory is clean
const status = run("git status --porcelain", { silent: true }).trim();
if (status) {
  console.error("\nWorking directory is not clean. Commit or stash changes first.\n");
  console.error(status);
  process.exit(1);
}

// 2. Bump version
const pkg = JSON.parse(readFileSync("package.json", "utf8"));
const [major, minor, patch] = pkg.version.split(".").map(Number);
const newVersion =
  bump === "major" ? `${major + 1}.0.0` :
  bump === "minor" ? `${major}.${minor + 1}.0` :
  `${major}.${minor}.${patch + 1}`;

console.log(`\nBumping ${pkg.version} → ${newVersion}\n`);

pkg.version = newVersion;
writeFileSync("package.json", JSON.stringify(pkg, null, 2) + "\n");

// 3. Update CHANGELOG.md — move [Unreleased] to the new version
const today = new Date().toISOString().slice(0, 10);
try {
  let changelog = readFileSync("CHANGELOG.md", "utf8");
  changelog = changelog.replace(
    /^## \[Unreleased\]/m,
    `## [Unreleased]\n\n## [${newVersion}] - ${today}`
  );
  writeFileSync("CHANGELOG.md", changelog);
  run(`git add CHANGELOG.md`);
} catch {
  // No CHANGELOG.md — that's fine
}

// 4. Commit + tag
run(`git add package.json`);
run(`git commit -m "chore: release v${newVersion}"`);
run(`git tag v${newVersion}`);

// 5. Push — this triggers GitHub Actions
console.log("\nPushing to origin...");
run(`git push origin main`);
run(`git push origin v${newVersion}`);

console.log(`
  Released v${newVersion}

  GitHub Actions is now:
    - Building binaries for all 5 platforms (Linux x64/arm64, macOS x64/arm64, Windows x64)
    - Creating a GitHub Release with the archives
    - Publishing beehive-cli@${newVersion} to npm

  Watch the build: https://github.com/mahaddev-x/beehive/actions
`);
