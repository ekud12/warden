#!/usr/bin/env node
// Warden npm/bun postinstall — downloads the platform-specific binary
// to ~/.warden/bin/ and registers PATH.

const { execSync, spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");
const https = require("https");

const VERSION = require("../package.json").version;
const REPO = "ekud12/warden";

const PLATFORM_MAP = {
  "win32-x64": "warden-x86_64-pc-windows-msvc.exe",
  "win32-arm64": "warden-aarch64-pc-windows-msvc.exe",
  "darwin-x64": "warden-x86_64-apple-darwin",
  "darwin-arm64": "warden-aarch64-apple-darwin",
  "linux-x64": "warden-x86_64-unknown-linux-gnu",
  "linux-arm64": "warden-aarch64-unknown-linux-gnu",
};

async function main() {
  const platform = `${os.platform()}-${os.arch()}`;
  const binary = PLATFORM_MAP[platform];

  if (!binary) {
    console.error(`Unsupported platform: ${platform}`);
    console.error("Install from source: cargo install warden-ai");
    process.exit(0); // Don't fail npm install
  }

  const wardenHome = path.join(os.homedir(), ".warden");
  const binDir = path.join(wardenHome, "bin");
  const destName = os.platform() === "win32" ? "warden.exe" : "warden";
  const dest = path.join(binDir, destName);

  // Create directories
  fs.mkdirSync(binDir, { recursive: true });
  fs.mkdirSync(path.join(wardenHome, "rules"), { recursive: true });
  fs.mkdirSync(path.join(wardenHome, "projects"), { recursive: true });

  // Download binary from GitHub Releases
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${binary}`;
  console.log(`Downloading warden v${VERSION} for ${platform}...`);

  try {
    await download(url, dest);
    if (os.platform() === "win32") {
      // Remove Zone.Identifier to prevent SmartScreen "Access denied"
      try { fs.unlinkSync(dest + ":Zone.Identifier"); } catch (e) {}
    } else {
      fs.chmodSync(dest, 0o755);
    }

    // Also download the relay binary (Windows only — prevents CMD flicker)
    if (os.platform() === "win32") {
      const relayBinary = binary.replace("warden-", "warden-relay-");
      const relayDest = path.join(binDir, "warden-relay.exe");
      const relayUrl = `https://github.com/${REPO}/releases/download/v${VERSION}/${relayBinary}`;
      try {
        await download(relayUrl, relayDest);
        try { fs.unlinkSync(relayDest + ":Zone.Identifier"); } catch (e) {}
        console.log(`Installed relay to ${relayDest}`);
      } catch (e) {
        // Relay is optional — warden works without it (just has CMD flicker)
      }
    }

    console.log(`Installed to ${dest}`);

    // Run warden init (non-interactive: just PATH + config)
    try {
      spawnSync(dest, ["version"], { stdio: "inherit" });
    } catch (e) {
      // Ignore — binary might need different setup
    }

    console.log("");
    console.log("Run 'warden init' to complete setup (install CLI tools, configure hooks).");
    console.log(`Or: 'warden install claude-code' / 'warden install gemini-cli'`);
  } catch (err) {
    console.error(`Download failed: ${err.message}`);
    console.error("Install from source: cargo install warden-ai");
    process.exit(0); // Don't fail npm install
  }
}

function download(url, dest, redirects = 0) {
  if (redirects > 10) return Promise.reject(new Error("Too many redirects"));
  return new Promise((resolve, reject) => {
    const proto = url.startsWith("https") ? https : require("http");
    proto.get(url, (response) => {
      if (response.statusCode === 301 || response.statusCode === 302) {
        response.resume(); // drain the response
        return resolve(download(response.headers.location, dest, redirects + 1));
      }
      if (response.statusCode !== 200) {
        response.resume();
        return reject(new Error(`HTTP ${response.statusCode} for ${url}`));
      }
      const file = fs.createWriteStream(dest);
      response.pipe(file);
      file.on("finish", () => { file.close(); resolve(); });
      file.on("error", (e) => { fs.unlinkSync(dest); reject(e); });
    }).on("error", reject);
  });
}

main().catch((e) => {
  console.error(e.message);
  process.exit(0);
});
