import type { Command } from "commander";
import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(readFileSync(join(__dirname, "../../package.json"), "utf8"));
const CURRENT_VERSION: string = pkg.version;

const REPO = "mahaddev-x/beehive";

async function getLatestVersion(): Promise<string | null> {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers: { "User-Agent": "beehive-cli" },
      signal: AbortSignal.timeout(5_000),
    });
    if (!res.ok) return null;
    const data = (await res.json()) as { tag_name?: string };
    return data.tag_name?.replace(/^v/, "") ?? null;
  } catch {
    return null;
  }
}

function isNewer(latest: string, current: string): boolean {
  const a = latest.split(".").map(Number);
  const b = current.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    if ((a[i] ?? 0) > (b[i] ?? 0)) return true;
    if ((a[i] ?? 0) < (b[i] ?? 0)) return false;
  }
  return false;
}

export function registerUpdateCommand(program: Command) {
  program
    .command("update")
    .description("Check for updates and install the latest version")
    .action(async () => {
      console.log(`\n  Checking for updates... (current: v${CURRENT_VERSION})\n`);

      const latest = await getLatestVersion();

      if (!latest) {
        console.log("  Could not reach GitHub. Check your connection.\n");
        return;
      }

      if (!isNewer(latest, CURRENT_VERSION)) {
        console.log(`  Already up to date (v${CURRENT_VERSION}).\n`);
        return;
      }

      console.log(`  New version available: v${latest}\n`);
      console.log("  Installing update...\n");

      if (process.platform === "win32") {
        // Re-run the PowerShell install script — it overwrites the binary
        try {
          execSync(
            `powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://mahaddev-x.github.io/beehive/install.ps1 | iex"`,
            { stdio: "inherit" }
          );
        } catch {
          console.log("  Update failed. Run manually:\n");
          console.log("    irm https://mahaddev-x.github.io/beehive/install.ps1 | iex\n");
        }
      } else {
        // Re-run the shell install script
        try {
          execSync(
            `curl -fsSL https://mahaddev-x.github.io/beehive/install.sh | bash`,
            { stdio: "inherit" }
          );
        } catch {
          console.log("  Update failed. Run manually:\n");
          console.log("    curl -fsSL https://mahaddev-x.github.io/beehive/install.sh | bash\n");
        }
      }
    });
}

/** Silent background check — prints a one-line notice if an update is available. */
export async function checkForUpdateNotice(): Promise<void> {
  const latest = await getLatestVersion();
  if (latest && isNewer(latest, CURRENT_VERSION)) {
    console.log(`\n  Update available: v${CURRENT_VERSION} → v${latest}   Run: beehive update\n`);
  }
}
