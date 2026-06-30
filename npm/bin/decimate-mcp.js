#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");
const exeName = process.platform === "win32" ? "decimate-mcp.exe" : "decimate-mcp";
const cachedBinary = path.join(root, "npm", "bin-cache", exeName);
const releaseBinary = path.join(root, "target", "release", exeName);
const debugBinary = path.join(root, "target", "debug", exeName);
const args = process.argv.slice(2);

for (const candidate of [cachedBinary, releaseBinary, debugBinary]) {
  if (fs.existsSync(candidate)) {
    run(candidate, args);
  }
}

const cargo = process.env.CARGO || "cargo";
run(cargo, ["run", "--release", "--locked", "--bin", "decimate-mcp", "--", ...args], root);

function run(command, commandArgs, cwd = undefined) {
  const result = spawnSync(command, commandArgs, {
    cwd,
    stdio: "inherit",
    windowsHide: false
  });

  if (result.error) {
    if (result.error.code === "ENOENT") {
      console.error(
        "decimate-mcp: Rust/Cargo is required to build the npm source package. " +
          "Install Rust from https://rustup.rs or use a release package with a prebuilt binary."
      );
      process.exit(127);
    }
    console.error(`decimate-mcp: failed to execute ${command}: ${result.error.message}`);
    process.exit(1);
  }

  if (result.signal) {
    process.kill(process.pid, result.signal);
    return;
  }

  process.exit(result.status ?? 1);
}
