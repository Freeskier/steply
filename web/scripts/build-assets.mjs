import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const webDir = resolve(scriptDir, "..");
const generatedDir = resolve(webDir, "src", "lib", "generated");

mkdirSync(generatedDir, { recursive: true });

run("wasm-pack", [
  "build",
  "../../crates/steply-wasm",
  "--target",
  "nodejs",
  "--out-dir",
  "./src/lib/steply-wasm/pkg",
  "--out-name",
  "steply_wasm",
]);

run("cargo", [
  "run",
  "-p",
  "steply-cli",
  "--",
  "export-schema",
  "--out",
  resolve(generatedDir, "config.schema.json"),
]);

run("cargo", [
  "run",
  "-p",
  "steply-cli",
  "--",
  "export-docs",
  "--out",
  resolve(generatedDir, "config.docs.json"),
]);

function run(command, args) {
  execFileSync(command, args, {
    cwd: webDir,
    stdio: "inherit",
    env: process.env,
  });
}
