#!/usr/bin/env node
// Warden npm/bun postinstall — downloads the platform-specific binary
// to ~/.warden/bin/ and registers PATH.

const { spawnSync } = require("child_process");
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
  const ext = os.platform() === "win32" ? ".exe" : "";
  const dest = path.join(binDir, `warden${ext}`);

  // Create directories
  fs.mkdirSync(binDir, { recursive: true });
  fs.mkdirSync(path.join(wardenHome, "rules"), { recursive: true });
  fs.mkdirSync(path.join(wardenHome, "projects"), { recursive: true });

  // Strategy 1: Check if warden is already installed via cargo and is current version
  const localCargo = findCargoBinary();
  if (localCargo) {
    console.log(`Found cargo-installed warden at ${localCargo}`);
    if (localCargo !== dest) {
      fs.copyFileSync(localCargo, dest);
      console.log(`Copied to ${dest}`);
    }
    copyRelay(binDir, localCargo);
    postInstall(dest);
    return;
  }

  // Strategy 2: Download from GitHub Releases
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${binary}`;
  console.log(`Downloading warden v${VERSION} for ${platform}...`);

  try {
    await download(url, dest);
    const stat = fs.statSync(dest);
    if (stat.size === 0) {
      fs.unlinkSync(dest);
      throw new Error("Downloaded file is empty (asset may not exist in release)");
    }

    if (os.platform() === "win32") {
      try { fs.unlinkSync(dest + ":Zone.Identifier"); } catch (e) {}
    } else {
      fs.chmodSync(dest, 0o755);
    }

    // Download relay binary (Windows only — prevents CMD flicker)
    if (os.platform() === "win32") {
      const relayBinary = binary.replace("warden-", "warden-relay-");
      const relayDest = path.join(binDir, "warden-relay.exe");
      const relayUrl = `https://github.com/${REPO}/releases/download/v${VERSION}/${relayBinary}`;
      try {
        await download(relayUrl, relayDest);
        const relayStat = fs.statSync(relayDest);
        if (relayStat.size === 0) {
          fs.unlinkSync(relayDest);
        } else {
          try { fs.unlinkSync(relayDest + ":Zone.Identifier"); } catch (e) {}
          console.log(`Installed relay to ${relayDest}`);
        }
      } catch (e) {
        // Relay is optional — warden works without it (just has CMD flicker)
      }
    }

    postInstall(dest);
  } catch (err) {
    // Clean up empty/partial file
    try { if (fs.existsSync(dest) && fs.statSync(dest).size === 0) fs.unlinkSync(dest); } catch (e) {}

    console.error(`Download failed: ${err.message}`);
    console.error("");
    console.error("Alternative install methods:");
    console.error("  cargo install warden-ai");
    console.error("  Download from: https://github.com/ekud12/warden/releases");
    process.exit(0); // Don't fail npm install
  }
}

function postInstall(dest) {
  console.log(`Installed to ${dest}`);

  // Verify binary works
  try {
    spawnSync(dest, ["version"], { stdio: "inherit", timeout: 5000 });
  } catch (e) {
    // Ignore — binary might need different setup
  }

  console.log("");
  console.log("Run 'warden init' to complete setup (install CLI tools, configure hooks).");
  console.log("Or: 'warden install claude-code' / 'warden install gemini-cli'");
}

function findCargoBinary() {
  // Check common cargo install locations
  const ext = os.platform() === "win32" ? ".exe" : "";
  const candidates = [
    path.join(os.homedir(), ".cargo", "bin", `warden${ext}`),
  ];

  // Also check if there's a local build in the project (for development)
  const envBinary = process.env.WARDEN_BINARY;
  if (envBinary && fs.existsSync(envBinary)) {
    return envBinary;
  }

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      const stat = fs.statSync(candidate);
      if (stat.size > 0) return candidate;
    }
  }
  return null;
}

function copyRelay(binDir, sourceBinaryDir) {
  if (os.platform() !== "win32") return;

  const sourceDir = path.dirname(sourceBinaryDir);
  const relaySource = path.join(sourceDir, "warden-relay.exe");
  const relayDest = path.join(binDir, "warden-relay.exe");

  if (fs.existsSync(relaySource) && relaySource !== relayDest) {
    try {
      fs.copyFileSync(relaySource, relayDest);
      console.log(`Copied relay to ${relayDest}`);
    } catch (e) {
      // Non-fatal
    }
  }
}

function download(url, dest, redirects = 0) {
  if (redirects > 10) return Promise.reject(new Error("Too many redirects"));
  return new Promise((resolve, reject) => {
    const proto = url.startsWith("https") ? https : require("http");
    proto.get(url, (response) => {
      if (response.statusCode === 301 || response.statusCode === 302) {
        response.resume();
        return resolve(download(response.headers.location, dest, redirects + 1));
      }
      if (response.statusCode !== 200) {
        response.resume();
        return reject(new Error(`HTTP ${response.statusCode} for ${url}`));
      }
      const file = fs.createWriteStream(dest);
      response.pipe(file);
      file.on("finish", () => { file.close(); resolve(); });
      file.on("error", (e) => {
        try { fs.unlinkSync(dest); } catch (ignore) {}
        reject(e);
      });
    }).on("error", (e) => {
      try { fs.unlinkSync(dest); } catch (ignore) {}
      reject(e);
    });
  });
}

main().catch((e) => {
  console.error(e.message);
  process.exit(0);
});
