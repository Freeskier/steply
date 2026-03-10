import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "../..");
const docsOut = resolve(repoRoot, "web/src/lib/generated/config.docs.json");
const schemaOut = resolve(repoRoot, "web/static/schema/steply.schema.json");

mkdirSync(dirname(docsOut), { recursive: true });
mkdirSync(dirname(schemaOut), { recursive: true });

run("cargo", ["run", "-p", "steply-cli", "--", "export-docs", "--out", docsOut]);
run("cargo", ["run", "-p", "steply-cli", "--", "export-schema", "--out", schemaOut]);

function run(command, args) {
	const result = spawnSync(command, args, {
		cwd: repoRoot,
		stdio: "inherit"
	});

	if ((result.status ?? 1) !== 0) {
		process.exit(result.status ?? 1);
	}
}
