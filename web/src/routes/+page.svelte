<script lang="ts">
	import { browser } from "$app/environment";
	import { previewDocsJson } from "$lib";

	let status = "WASM not loaded";
	let output = "";
	let loading = false;
	let selectedInstallTab = "run-unix";
	let copiedLabel = "";
	let copiedReset: ReturnType<typeof setTimeout> | null = null;

	const fallbackOrigin = "https://steply.sh";
	const releaseLinks = [
		{
			label: "Linux x64",
			href: "https://github.com/Freeskier/steply/releases/latest/download/steply-x86_64-unknown-linux-gnu.tar.gz",
		},
		{
			label: "macOS arm64",
			href: "https://github.com/Freeskier/steply/releases/latest/download/steply-aarch64-apple-darwin.tar.gz",
		},
		{
			label: "Windows x64",
			href: "https://github.com/Freeskier/steply/releases/latest/download/steply-x86_64-pc-windows-msvc.zip",
		},
	];

	function siteUrl(path: string) {
		const origin = browser ? window.location.origin : fallbackOrigin;
		return new URL(path, origin).toString();
	}

	function installTabs() {
		return [
			{
				id: "run-unix",
				label: "Run",
				platform: "Unix",
				value: `curl -fsSL ${siteUrl("/run")} | sh -s -- --config config.yml`,
			},
			{
				id: "install-unix",
				label: "Install",
				platform: "Unix",
				value: `curl -fsSL ${siteUrl("/install")} | sh`,
			},
			{
				id: "run-windows",
				label: "Run",
				platform: "Windows",
				value: `powershell -ExecutionPolicy Bypass -Command "irm ${siteUrl("/run.ps1")} | iex"`,
			},
			{
				id: "install-windows",
				label: "Install",
				platform: "Windows",
				value: `powershell -ExecutionPolicy Bypass -Command "irm ${siteUrl("/install.ps1")} | iex"`,
			},
		];
	}

	function activeInstallTab() {
		return (
			installTabs().find((tab) => tab.id === selectedInstallTab) ??
			installTabs()[0]
		);
	}

	async function copyText(label: string, value: string) {
		if (!browser || !navigator.clipboard) {
			return;
		}
		await navigator.clipboard.writeText(value);
		copiedLabel = label;
		if (copiedReset) clearTimeout(copiedReset);
		copiedReset = setTimeout(() => {
			copiedLabel = "";
		}, 1800);
	}

	async function runWasmDemo() {
		loading = true;
		status = "Loading WASM...";
		output = "";

		try {
			const docsJson = await previewDocsJson();
			status = "WASM loaded";
			output =
				docsJson.slice(0, 800) +
				(docsJson.length > 800 ? "\n\n...trimmed" : "");
		} catch (error) {
			status = "WASM error";
			output = error instanceof Error ? error.message : String(error);
		} finally {
			loading = false;
		}
	}
</script>

<main>
	<section class="hero">
		<div class="hero-copy">
			<p class="eyebrow">Flow runtime for the command line</p>
			<p class="lead">
				Steply feels like a shell prompt that learned composition, state, and
				structure. The mark leans into that: a hot signal rail stepping upward
				through a controlled path instead of a soft generic blob.
			</p>

			<div class="actions">
				<button on:click={runWasmDemo} disabled={loading}>
					{loading ? "Loading..." : "Run WASM demo"}
				</button>
				<span class="status">{status}</span>
			</div>
		</div>
	</section>

	<section class="install-panel">
		<div class="panel-copy">
			<p class="panel-kicker">Launch from the latest GitHub release</p>
			<h2>Run or install Steply without hand-picking assets.</h2>
			<p class="panel-lead">
				The launcher scripts resolve the newest release, choose the right build
				for the current machine, and either install it or execute it from a temp
				directory.
			</p>
		</div>

		<div class="panel-terminal">
			<div class="tab-row">
				{#each installTabs() as tab}
					<button
						class:selected={tab.id === selectedInstallTab}
						class="tab"
						type="button"
						on:click={() => (selectedInstallTab = tab.id)}
					>
						{tab.label} <span>{tab.platform}</span>
					</button>
				{/each}
			</div>

			<div class="command-card">
				<div class="command-head">
					<div>
						<strong>{activeInstallTab().label}</strong>
						<span>{activeInstallTab().platform}</span>
					</div>
					<button
						class="copy"
						type="button"
						on:click={() =>
							copyText(activeInstallTab().label, activeInstallTab().value)}
					>
						{copiedLabel === activeInstallTab().label ? "Copied" : "Copy"}
					</button>
				</div>
				<pre class="command">{activeInstallTab().value}</pre>
			</div>
		</div>
	</section>

	<section class="downloads">
		<div class="downloads-head">
			<p class="panel-kicker">Direct release assets</p>
			<h3>Copy URLs or grab the platform archive directly.</h3>
		</div>
		<div class="download-grid">
			{#each releaseLinks as link}
				<div class="download-item">
					<div>
						<strong>{link.label}</strong>
						<a href={link.href} target="_blank" rel="noreferrer">{link.href}</a>
					</div>
					<button
						type="button"
						class="copy"
						on:click={() => copyText(link.label, link.href)}
					>
						{copiedLabel === link.label ? "Copied" : "Copy"}
					</button>
				</div>
			{/each}
		</div>
	</section>

	{#if output}
		<section class="console">
			<div class="console-head">
				<span>preview_config_docs_json()</span>
				<span>client wasm</span>
			</div>
			<pre>{output}</pre>
		</section>
	{/if}
</main>

<style>
	:global(body) {
		margin: 0;
		background:
			radial-gradient(
				circle at top left,
				rgba(255, 183, 41, 0.18),
				transparent 28%
			),
			radial-gradient(
				circle at 85% 18%,
				rgba(255, 122, 26, 0.14),
				transparent 24%
			),
			linear-gradient(180deg, #f6f1e7 0%, #efe1c6 48%, #e6d4b1 100%);
		color: #15110d;
	}

	main {
		max-width: 1180px;
		margin: 0 auto;
		min-height: 100vh;
		padding: 3rem 1.25rem 4rem;
		font-family: "Avenir Next", "Segoe UI", Helvetica, Arial, sans-serif;
	}

	.hero {
		display: grid;
		grid-template-columns: minmax(0, 1.15fr) minmax(280px, 420px);
		gap: 2rem;
		align-items: center;
	}

	.eyebrow {
		margin: 0 0 1rem;
		font-size: 0.78rem;
		font-weight: 700;
		letter-spacing: 0.22em;
		text-transform: uppercase;
		color: #7a4b18;
	}

	.lead {
		max-width: 56ch;
		margin: 1.25rem 0 0;
		font-size: clamp(1.05rem, 1.4vw, 1.28rem);
		line-height: 1.6;
		color: #3a2a16;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem;
		align-items: center;
		margin-top: 1.8rem;
	}

	button {
		padding: 0.88rem 1.25rem;
		font-size: 0.98rem;
		font-weight: 700;
		letter-spacing: 0.01em;
		cursor: pointer;
		border: 0;
		border-radius: 999px;
		color: #fff7e7;
		background: linear-gradient(135deg, #1f160f 0%, #503212 46%, #c45b0c 100%);
		box-shadow: 0 14px 28px rgba(104, 50, 5, 0.18);
	}

	button:disabled {
		cursor: progress;
		opacity: 0.76;
	}

	.status {
		font-size: 0.95rem;
		font-weight: 700;
		color: #69431e;
	}

	.console {
		margin-top: 2.5rem;
		border-radius: 28px;
		overflow: hidden;
		background: #130f0c;
		box-shadow: 0 30px 70px rgba(26, 15, 5, 0.22);
	}

	.console-head {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.95rem 1.2rem;
		background: linear-gradient(180deg, #2b211a, #1d1510);
		color: #efc87b;
		font-size: 0.85rem;
		letter-spacing: 0.05em;
		text-transform: uppercase;
	}

	pre {
		margin: 0;
		white-space: pre-wrap;
		word-break: break-word;
		padding: 1.2rem;
		color: #f2e4c7;
		background: transparent;
		font-size: 0.92rem;
		line-height: 1.55;
	}

	.install-panel {
		margin-top: 2.75rem;
		display: grid;
		grid-template-columns: minmax(0, 0.86fr) minmax(0, 1.14fr);
		gap: 1.5rem;
		align-items: start;
	}

	.panel-kicker {
		margin: 0 0 0.9rem;
		font-size: 0.78rem;
		font-weight: 700;
		letter-spacing: 0.18em;
		text-transform: uppercase;
		color: #8b5f22;
	}

	.panel-copy h2,
	.downloads-head h3 {
		margin: 0;
		font-size: clamp(1.7rem, 2.5vw, 2.5rem);
		line-height: 1.05;
		color: #17120c;
	}

	.panel-lead {
		max-width: 54ch;
		margin: 1rem 0 0;
		font-size: 1rem;
		line-height: 1.7;
		color: #4b3720;
	}

	.panel-terminal {
		border-radius: 30px;
		overflow: hidden;
		background: linear-gradient(
			180deg,
			rgba(19, 24, 34, 0.98),
			rgba(10, 14, 22, 0.98)
		);
		box-shadow: 0 30px 70px rgba(15, 14, 19, 0.24);
	}

	.tab-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0;
		border-bottom: 1px solid rgba(255, 255, 255, 0.07);
	}

	.tab {
		border-radius: 0;
		background: transparent;
		box-shadow: none;
		color: #9098a9;
		padding: 0.9rem 1rem;
		border-right: 1px solid rgba(255, 255, 255, 0.06);
	}

	.tab span {
		color: #646f82;
		margin-left: 0.45rem;
	}

	.tab.selected {
		background: #ffc52e;
		color: #12161f;
	}

	.tab.selected span {
		color: #524114;
	}

	.command-card {
		padding: 1.15rem;
	}

	.command-head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 1rem;
		margin-bottom: 0.85rem;
		color: #ecf1fa;
	}

	.command-head span {
		display: inline-block;
		margin-left: 0.75rem;
		color: #748096;
		font-size: 0.9rem;
	}

	.copy {
		padding: 0.62rem 0.88rem;
		font-size: 0.86rem;
		font-weight: 700;
		border-radius: 999px;
		background: #1b2230;
		color: #eff4ff;
		border: 1px solid rgba(255, 255, 255, 0.08);
		box-shadow: none;
	}

	.command {
		padding: 1rem 1.05rem;
		border-radius: 20px;
		background: #0a0e15;
		border: 1px solid rgba(255, 197, 46, 0.14);
		color: #ffd466;
		font-size: 0.96rem;
		line-height: 1.65;
	}

	.downloads {
		margin-top: 2rem;
		padding: 1.35rem;
		border-radius: 28px;
		background: rgba(255, 248, 233, 0.55);
		box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.68);
	}

	.download-grid {
		display: grid;
		gap: 0.9rem;
		margin-top: 1rem;
	}

	.download-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 1rem;
		padding: 1rem;
		border-radius: 20px;
		background: rgba(255, 255, 255, 0.5);
		border: 1px solid rgba(74, 55, 32, 0.08);
	}

	.download-item strong {
		display: block;
		margin-bottom: 0.35rem;
		color: #17120c;
	}

	.download-item a {
		color: #6c4714;
		font-size: 0.94rem;
		word-break: break-all;
		text-decoration: none;
	}

	@media (max-width: 900px) {
		.hero {
			grid-template-columns: 1fr;
		}

		.install-panel {
			grid-template-columns: 1fr;
		}

		.download-item {
			flex-direction: column;
			align-items: stretch;
		}
	}
</style>
