import { spawnSync } from "node:child_process";
import { rmSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "../..");
const outDir = resolve(repoRoot, "web/src/lib/steply-wasm/pkg");

run("wasm-pack", [
	"build",
	resolve(repoRoot, "crates/steply-wasm"),
	"--target",
	"web",
	"--out-dir",
	outDir,
	"--out-name",
	"steply_wasm"
]);

rmSync(resolve(outDir, ".gitignore"), { force: true });

function run(command, args) {
	const result = spawnSync(command, args, {
		cwd: repoRoot,
		stdio: "inherit"
	});

	if ((result.status ?? 1) !== 0) {
		process.exit(result.status ?? 1);
	}
}
