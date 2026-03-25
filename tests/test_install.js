#!/usr/bin/env node
// Local install test — simulates what npm postinstall, cargo install, and
// warden install claude-code would do, using local binaries.
//
// Usage:
//   node tests/test_install.js [test_name]
//
// Tests:
//   npm-postinstall   — simulates postinstall with local binary (WARDEN_BINARY env)
//   cargo-install     — simulates cargo install by copying to ~/.cargo/bin/
//   warden-init       — runs warden init (interactive wizard)
//   warden-install    — runs warden install claude-code
//   warden-uninstall  — runs warden uninstall claude-code
//   warden-version    — runs warden version
//   bin-wrapper       — tests the npm bin/warden wrapper
//   all               — runs all non-interactive tests
//
// Prerequisites: cargo build --release

const { spawnSync, execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");

const PROJECT_ROOT = path.resolve(__dirname, "..");
const RELEASE_DIR = path.join(PROJECT_ROOT, "target", "release");
const ext = os.platform() === "win32" ? ".exe" : "";
const WARDEN_BIN = path.join(RELEASE_DIR, `warden${ext}`);
const RELAY_BIN = path.join(RELEASE_DIR, `warden-relay${ext}`);
const WARDEN_HOME = path.join(os.homedir(), ".warden");
const WARDEN_BIN_DIR = path.join(WARDEN_HOME, "bin");

const RED = "\x1b[31m";
const GREEN = "\x1b[32m";
const YELLOW = "\x1b[33m";
const CYAN = "\x1b[36m";
const RESET = "\x1b[0m";

function log(color, prefix, msg) {
  console.log(`${color}[${prefix}]${RESET} ${msg}`);
}

function checkPrereqs() {
  if (!fs.existsSync(WARDEN_BIN)) {
    log(RED, "FAIL", `Binary not found: ${WARDEN_BIN}`);
    log(YELLOW, "HINT", "Run: cargo build --release");
    process.exit(1);
  }
  const stat = fs.statSync(WARDEN_BIN);
  log(GREEN, "OK", `Binary found: ${WARDEN_BIN} (${(stat.size / 1024 / 1024).toFixed(1)}MB)`);

  if (os.platform() === "win32" && fs.existsSync(RELAY_BIN)) {
    const relayStat = fs.statSync(RELAY_BIN);
    log(GREEN, "OK", `Relay found: ${RELAY_BIN} (${(relayStat.size / 1024).toFixed(0)}KB)`);
  }
}

function run(cmd, args, opts = {}) {
  log(CYAN, "RUN", `${cmd} ${args.join(" ")}`);
  const result = spawnSync(cmd, args, {
    stdio: opts.inherit ? "inherit" : "pipe",
    timeout: opts.timeout || 10000,
    env: { ...process.env, ...opts.env },
    windowsHide: true,
  });
  if (result.error) {
    log(RED, "ERR", `${result.error.code}: ${result.error.message}`);
    return { ok: false, error: result.error };
  }
  const stdout = result.stdout ? result.stdout.toString().trim() : "";
  const stderr = result.stderr ? result.stderr.toString().trim() : "";
  if (result.status !== 0) {
    log(RED, "FAIL", `Exit ${result.status}: ${stderr || stdout}`);
    return { ok: false, status: result.status, stdout, stderr };
  }
  if (stdout) log(GREEN, "OUT", stdout.split("\n")[0]);
  return { ok: true, status: 0, stdout, stderr };
}

// --- Tests ---

function testNpmPostinstall() {
  log(CYAN, "TEST", "npm postinstall (local binary via WARDEN_BINARY env)");

  // Run the postinstall script with WARDEN_BINARY pointing to local build
  const result = run("node", [
    path.join(PROJECT_ROOT, "packages", "npm", "scripts", "postinstall.js")
  ], {
    env: { WARDEN_BINARY: WARDEN_BIN },
    inherit: true,
    timeout: 15000,
  });

  // Verify binary was copied
  const dest = path.join(WARDEN_BIN_DIR, `warden${ext}`);
  if (fs.existsSync(dest) && fs.statSync(dest).size > 0) {
    log(GREEN, "PASS", `Binary installed at ${dest}`);
  } else {
    log(RED, "FAIL", `Binary missing or empty at ${dest}`);
    return false;
  }

  // Verify relay on Windows
  if (os.platform() === "win32") {
    const relayDest = path.join(WARDEN_BIN_DIR, "warden-relay.exe");
    if (fs.existsSync(relayDest) && fs.statSync(relayDest).size > 0) {
      log(GREEN, "PASS", `Relay installed at ${relayDest}`);
    } else {
      log(YELLOW, "WARN", `Relay missing at ${relayDest} (optional)`);
    }
  }

  return result.ok;
}

function testCargoInstall() {
  log(CYAN, "TEST", "cargo install (copy to ~/.cargo/bin/)");

  const cargoDir = path.join(os.homedir(), ".cargo", "bin");
  fs.mkdirSync(cargoDir, { recursive: true });

  const dest = path.join(cargoDir, `warden${ext}`);
  fs.copyFileSync(WARDEN_BIN, dest);
  if (os.platform() !== "win32") fs.chmodSync(dest, 0o755);

  const stat = fs.statSync(dest);
  log(GREEN, "PASS", `Copied to ${dest} (${(stat.size / 1024 / 1024).toFixed(1)}MB)`);

  // Verify it runs
  const result = run(dest, ["version"]);
  return result.ok;
}

function testWardenVersion() {
  log(CYAN, "TEST", "warden version");
  const result = run(WARDEN_BIN, ["version"]);
  return result.ok;
}

function testWardenInstall() {
  log(CYAN, "TEST", "warden install claude-code");

  // Backup current settings.json
  const settingsPath = path.join(os.homedir(), ".claude", "settings.json");
  let backup = null;
  if (fs.existsSync(settingsPath)) {
    backup = fs.readFileSync(settingsPath, "utf8");
    log(YELLOW, "INFO", `Backed up ${settingsPath}`);
  }

  const result = run(WARDEN_BIN, ["install", "claude-code"], {
    inherit: true,
    timeout: 15000,
  });

  // Show what changed
  if (fs.existsSync(settingsPath)) {
    const after = fs.readFileSync(settingsPath, "utf8");
    if (backup !== after) {
      log(GREEN, "PASS", "settings.json was modified");
      // Show the hooks section
      try {
        const json = JSON.parse(after);
        const hookCount = Object.keys(json.hooks || {}).length;
        log(GREEN, "INFO", `${hookCount} hook categories configured`);
      } catch (e) {}
    } else {
      log(YELLOW, "WARN", "settings.json unchanged (hooks may already be installed)");
    }
  }

  return result.ok;
}

function testWardenUninstall() {
  log(CYAN, "TEST", "warden uninstall (piping 'n' to skip directory removal)");
  // Uninstall is now interactive (confirm prompt).
  // Pipe 'n' via stdin to decline directory removal.
  const { spawnSync: ss } = require("child_process");
  log(CYAN, "RUN", `${WARDEN_BIN} uninstall`);
  const result = ss(WARDEN_BIN, ["uninstall"], {
    input: "n\n",
    stdio: ["pipe", "pipe", "pipe"],
    timeout: 15000,
    windowsHide: true,
  });
  if (result.error) {
    log(RED, "ERR", `${result.error.code}: ${result.error.message}`);
    return false;
  }
  // Uninstall exits 0 even when declining directory removal
  log(GREEN, "OUT", "Uninstall completed (directory preserved)");
  return result.status === 0;
}

function testBinWrapper() {
  log(CYAN, "TEST", "npm bin/warden wrapper");
  const wrapper = path.join(PROJECT_ROOT, "packages", "npm", "bin", "warden");

  // Ensure binary exists in ~/.warden/bin/
  const dest = path.join(WARDEN_BIN_DIR, `warden${ext}`);
  if (!fs.existsSync(dest) || fs.statSync(dest).size === 0) {
    fs.mkdirSync(WARDEN_BIN_DIR, { recursive: true });
    fs.copyFileSync(WARDEN_BIN, dest);
    log(YELLOW, "INFO", `Copied binary to ${dest} for wrapper test`);
  }

  const result = run("node", [wrapper, "version"]);
  return result.ok;
}

function testWardenInit() {
  log(CYAN, "TEST", "warden init (INTERACTIVE — will prompt you)");
  const result = run(WARDEN_BIN, ["init"], { inherit: true, timeout: 60000 });
  return result.ok;
}

// --- Runner ---

const tests = {
  "npm-postinstall": testNpmPostinstall,
  "cargo-install": testCargoInstall,
  "warden-version": testWardenVersion,
  "warden-install": testWardenInstall,
  "warden-uninstall": testWardenUninstall,
  "bin-wrapper": testBinWrapper,
  "warden-init": testWardenInit,
};

const nonInteractive = [
  "warden-version",
  "npm-postinstall",
  "cargo-install",
  "bin-wrapper",
  "warden-install",
  "warden-uninstall",
];

function main() {
  const arg = process.argv[2] || "all";

  checkPrereqs();
  console.log("");

  let toRun;
  if (arg === "all") {
    toRun = nonInteractive;
  } else if (tests[arg]) {
    toRun = [arg];
  } else {
    console.error(`Unknown test: ${arg}`);
    console.error(`Available: ${Object.keys(tests).join(", ")}, all`);
    process.exit(1);
  }

  const results = [];
  for (const name of toRun) {
    console.log(`\n${"=".repeat(60)}`);
    try {
      const passed = tests[name]();
      results.push({ name, passed });
    } catch (e) {
      log(RED, "CRASH", `${name}: ${e.message}`);
      results.push({ name, passed: false });
    }
  }

  console.log(`\n${"=".repeat(60)}`);
  console.log("Results:");
  let allPassed = true;
  for (const { name, passed } of results) {
    const icon = passed ? `${GREEN}PASS` : `${RED}FAIL`;
    console.log(`  ${icon}${RESET}  ${name}`);
    if (!passed) allPassed = false;
  }
  console.log("");

  if (allPassed) {
    log(GREEN, "ALL", `${results.length}/${results.length} tests passed`);
  } else {
    const failed = results.filter(r => !r.passed).length;
    log(RED, "FAIL", `${failed}/${results.length} tests failed`);
  }

  process.exit(allPassed ? 0 : 1);
}

main();
