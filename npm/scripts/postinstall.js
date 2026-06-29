#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

if (process.env.DECIMATE_SKIP_BUILD === "1") {
  process.exit(0);
}

const root = path.resolve(__dirname, "../..");
const exeName = process.platform === "win32" ? "decimate.exe" : "decimate";
const cargo = process.env.CARGO || "cargo";
const builtBinary = path.join(root, "target", "release", exeName);
const cacheDir = path.join(root, "npm", "bin-cache");
const cachedBinary = path.join(cacheDir, exeName);

const build = spawnSync(cargo, ["build", "--release", "--locked"], {
  cwd: root,
  stdio: "inherit",
  windowsHide: false
});

if (build.error) {
  if (build.error.code === "ENOENT") {
    console.error(
      "decimate: Rust/Cargo is required to install this npm source package. " +
        "Install Rust from https://rustup.rs."
    );
    process.exit(127);
  }
  console.error(`decimate: cargo build failed to start: ${build.error.message}`);
  process.exit(1);
}

if (build.status !== 0) {
  process.exit(build.status ?? 1);
}

fs.mkdirSync(cacheDir, { recursive: true });
fs.copyFileSync(builtBinary, cachedBinary);
if (process.platform !== "win32") {
  fs.chmodSync(cachedBinary, 0o755);
}
