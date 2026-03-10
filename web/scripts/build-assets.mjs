import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const webDir = resolve(scriptDir, "..");
const wasmOutDir = resolve(webDir, "src", "lib", "steply-wasm", "pkg");

run("wasm-pack", [
	"build",
	"../crates/steply-wasm",
	"--target",
	"web",
	"--out-dir",
	wasmOutDir,
	"--out-name",
	"steply_wasm",
]);

function run(command, args) {
	try {
		execFileSync(command, args, {
			cwd: webDir,
			stdio: "inherit",
			env: process.env,
		});
	} catch (error) {
		if (
			error &&
			typeof error === "object" &&
			"code" in error &&
			error.code === "ENOENT"
		) {
			throw new Error(
				`Missing "${command}" binary. Run "bun install" in web/ and try again.`,
			);
		}
		throw error;
	}
}
