<script lang="ts">
    type InstallTab = "curl" | "homebrew" | "npm" | "cargo";
    type ShowcaseTab = "yaml" | "shell" | "creator";

    type InstallEntry = {
        label: string;
        command: string;
        note: string;
        detail: string;
    };

    type ShowcaseEntry = {
        label: string;
        caption: string;
        code: string;
        terminalTitle: string;
        terminalLines: string[];
        chips: string[];
    };

    type Slab = {
        eyebrow: string;
        title: string;
        body: string;
        points: string[];
        stat: string;
        statLabel: string;
    };

    type EntryPoint = {
        title: string;
        body: string;
        snippet: string;
        footer: string;
    };

    type WidgetTile = {
        name: string;
        detail: string;
        span?: "wide" | "tall";
    };

    type WorkflowStep = {
        index: string;
        title: string;
        body: string;
        artifact: string;
    };

    type ExampleCard = {
        title: string;
        body: string;
        tags: string[];
        accent: string;
    };

    const installs: Record<InstallTab, InstallEntry> = {
        curl: {
            label: "curl",
            command: "curl -fsSL https://get.steply.dev/install.sh | sh",
            note: "Fastest path on a fresh machine.",
            detail: "Single-binary install for laptops, ephemeral runners, and minimal containers.",
        },
        homebrew: {
            label: "Homebrew",
            command: "brew install steply",
            note: "Best fit for local macOS developer tooling.",
            detail: "Good for teams already distributing terminal tools through Brew.",
        },
        npm: {
            label: "npm",
            command: "npm install -g steply",
            note: "Useful when Node already owns your tooling surface.",
            detail: "Fits docs sites, creator tooling, and shell helpers managed from one JS stack.",
        },
        cargo: {
            label: "cargo",
            command: "cargo install steply-cli",
            note: "Native path for Rust-heavy environments.",
            detail: "Ideal for contributors and teams already living close to the Rust workspace.",
        },
    };

    const showcases: Record<ShowcaseTab, ShowcaseEntry> = {
        yaml: {
            label: "YAML",
            caption:
                "Define a flow as a reviewable contract with typed fields, validation, and real step boundaries.",
            code: `steps:
  - id: basic
    title: Basic user data
    widgets:
      - type: text_input
        id: name
        label: Name
      - type: text_input
        id: email
        label: Email

  - id: project
    title: Project setup
    widgets:
      - type: file_browser
        id: project_dir
        label: Project directory`,
            terminalTitle: "flow.yaml -> steply run",
            terminalLines: [
                "Basic user data",
                "",
                "Name:  Ada Lovelace",
                "Email: ada@analytical.engine",
                "",
                "[Enter] next field   [Tab] focus",
            ],
            chips: ["Typed fields", "Step validation", "Reviewable config"],
        },
        shell: {
            label: "Shell",
            caption:
                "Build the same runtime incrementally from scripts when branching and environment checks belong in bash.",
            code: `flow_id="$(steply flow create --decorate)"

steply flow step "$flow_id" --title "Basic user data"
steply text-input --flow "$flow_id" --target user.name --label "Name"
steply text-input --flow "$flow_id" --target user.email --label "Email"

steply flow run "$flow_id"`,
            terminalTitle: "draft flow -> same renderer",
            terminalLines: [
                "Basic user data",
                "",
                "Name:  _",
                "Email:",
                "",
                "Store -> user.name, user.email",
            ],
            chips: ["Flow drafts", "Imperative branching", "Same renderer"],
        },
        creator: {
            label: "Creator",
            caption:
                "Author steps visually, inspect widget fields, and export real YAML instead of inventing a separate editor format.",
            code: `Flow outline
  ├─ Basic user data
  │   ├─ text_input / Name
  │   └─ text_input / Email
  └─ Project setup
      └─ file_browser / Project directory

Inspector
  required: true
  submit_target: project.directory`,
            terminalTitle: "creator preview",
            terminalLines: [
                "Project setup",
                "",
                "Project directory:",
                "~/workspace/steply",
                "",
                "Export -> flow.yaml",
            ],
            chips: ["Visual editing", "Inspector", "Export clean YAML"],
        },
    };

    const slabs: Slab[] = [
        {
            eyebrow: "Schema",
            title: "Schema-aware authoring",
            body: "Widget fields, docs, CLI flags, and editor validation come from the same model layer, so the authoring contract stays coherent.",
            points: [
                "Typed YAML fields",
                "Generated docs JSON",
                "Schema-backed editor hints",
            ],
            stat: "1",
            statLabel: "model layer",
        },
        {
            eyebrow: "Runtime",
            title: "Composable terminal runtime",
            body: "Steps, validation, focus flow, task hooks, and submit targets live in a real interaction engine instead of one-shot prompt glue.",
            points: [
                "Multi-widget steps",
                "Task-aware transitions",
                "Shell-built flows supported",
            ],
            stat: "28",
            statLabel: "widget surfaces",
        },
        {
            eyebrow: "Preview",
            title: "Browser parity without fake UI",
            body: "The web preview follows the same Rust and Wasm rendering path that shapes the terminal runtime, so previews stay honest.",
            points: [
                "Shared renderer model",
                "Terminal-first layout logic",
                "Creator-ready architecture",
            ],
            stat: "98%",
            statLabel: "preview parity",
        },
    ];

    const entryPoints: EntryPoint[] = [
        {
            title: "YAML flow",
            body: "Best when you want a declarative artifact that is easy to diff, review, and document.",
            snippet: `steps:\n  - id: deploy\n    widgets:\n      - type: confirm_input`,
            footer: "Installers, onboarding, operator UX",
        },
        {
            title: "Shell flow builder",
            body: "Best when orchestration already lives in bash and you want prompts, state, and steps without throwing away scripts.",
            snippet: `steply flow step \"$id\" --title \"Checks\"\nsteply text-input --flow \"$id\" --target env.branch`,
            footer: "Bootstrap scripts, CI helpers, release flows",
        },
        {
            title: "Creator",
            body: "Best when people should design flows visually but still export a clean, inspectable YAML result.",
            snippet: `Outline → Inspector → Terminal\nExport: flow.yaml`,
            footer: "Docs demos, internal tooling, rapid iteration",
        },
    ];

    const widgetTiles: WidgetTile[] = [
        {
            name: "Text Input",
            detail: "Single-line inputs with validation, completion, and submit targets.",
            span: "wide",
        },
        {
            name: "File Browser",
            detail: "Tree or list mode, filtering, path-aware navigation.",
            span: "tall",
        },
        {
            name: "Table",
            detail: "Editable rows with embedded widgets and structured navigation.",
        },
        {
            name: "Repeater",
            detail: "Build lists of records without hand-writing objects.",
        },
        {
            name: "Object Editor",
            detail: "Edit nested config values with typed insertions and structure-aware UX.",
            span: "wide",
        },
        {
            name: "Command Runner",
            detail: "Run tasks, react to runtime events, surface progress inside the flow.",
        },
        {
            name: "Task Log",
            detail: "Keep long-running work and logs inside the same terminal product surface.",
        },
        {
            name: "Tree View",
            detail: "Hierarchical selection with filtering and submit targets.",
        },
    ];

    const workflow: WorkflowStep[] = [
        {
            index: "01",
            title: "Author",
            body: "Start from YAML, shell, or the creator without changing the runtime contract underneath.",
            artifact: "YAML spec / draft flow / outline",
        },
        {
            index: "02",
            title: "Shape the interaction",
            body: "Compose steps, widgets, validation, and submit targets until the flow reads like a real terminal product.",
            artifact: "Widgets + conditions + tasks",
        },
        {
            index: "03",
            title: "Preview before shipping",
            body: "Use the web surface to inspect how the flow will actually render, not just how it looks on paper.",
            artifact: "Wasm-backed terminal stage",
        },
        {
            index: "04",
            title: "Run and scale",
            body: "Ship the same flow to the terminal, generate docs, and keep the model stable as the creator grows.",
            artifact: "CLI runtime + docs pipeline",
        },
    ];

    const examples: ExampleCard[] = [
        {
            title: "Project bootstrap wizard",
            body: "Collect metadata, choose a directory, apply template variables, and hand off to the generator without dropping context.",
            tags: ["YAML", "Filesystem", "Template init"],
            accent: "Featured",
        },
        {
            title: "Deployment confirmation flow",
            body: "Gate release steps, inspect environment context, require confirmation, and run tasks between transitions.",
            tags: ["Ops", "Validation", "Runtime tasks"],
            accent: "Shell",
        },
        {
            title: "Repository scanner",
            body: "Combine file browser, object editor, and command runner to review a repository before applying changes.",
            tags: ["Review", "Command runner", "Tree view"],
            accent: "Preview-ready",
        },
    ];

    let installTab = $state<InstallTab>("curl");
    let showcaseTab = $state<ShowcaseTab>("yaml");
    let copied = $state(false);

    async function copyInstallCommand() {
        try {
            await navigator.clipboard.writeText(installs[installTab].command);
            copied = true;
            setTimeout(() => {
                copied = false;
            }, 1200);
        } catch {
            copied = false;
        }
    }
</script>

<svelte:head>
    <title>Steply</title>
    <meta
        name="description"
        content="Build terminal flows with YAML, shell, and a browser preview driven by the same runtime."
    />
</svelte:head>

<div class="page-shell">
    <section class="hero-stage stage-grid">
        <div class="noise"></div>
        <div class="hero-halo hero-halo-left"></div>
        <div class="hero-halo hero-halo-right"></div>

        <header class="topbar">
            <a class="brand" href="/">
                <span class="brand-mark">s</span>
                <span class="brand-name">Steply</span>
            </a>

            <nav class="topnav">
                <a href="/creator">Creator</a>
                <a href="/docs">Docs</a>
                <a href="#assembly-line">How it works</a>
            </nav>

            <a class="topbar-cta" href="/creator">Open Creator ↗</a>
        </header>

        <div class="hero-inner">
            <div class="hero-copy">
                <div class="eyebrow-badge">
                    >_ Terminal flows, prompts, and previews from one engine
                </div>

                <h1>
                    Build terminal UX
                    <span>like a product.</span>
                </h1>

                <p class="hero-description">
                    Steply lets you define multi-step terminal flows in YAML,
                    assemble them from shell, and preview the same runtime in
                    the browser without drifting into a separate frontend-only
                    mock.
                </p>

                <div class="hero-actions">
                    <a class="primary-action" href="/creator">Open Creator ↗</a>
                    <a class="secondary-action" href="/docs">Read docs</a>
                </div>

                <div class="hero-signal-grid">
                    <div>
                        <strong>YAML-first</strong>
                        <span>Typed config that stays reviewable.</span>
                    </div>
                    <div>
                        <strong>Shell-friendly</strong>
                        <span>Build real steps from bash, not ad hoc prompts.</span>
                    </div>
                    <div>
                        <strong>Wasm preview</strong>
                        <span>Render the same runtime behavior in-browser.</span>
                    </div>
                    <div>
                        <strong>Runtime tasks</strong>
                        <span>Wire terminal UX to real work, not static forms.</span>
                    </div>
                </div>

                <div class="hero-foot">
                    <span>v0.9.2</span>
                    <span class="divider"></span>
                    <span>Schema-first</span>
                    <span>Composable</span>
                    <span>Deterministic</span>
                </div>
            </div>

            <div class="hero-stage-stack">
                <section class="install-dock surface-panel">
                    <div class="panel-head">
                        <div>
                            <p class="section-kicker">Install</p>
                            <h2>One command to get running.</h2>
                        </div>

                        <button
                            class="copy-button"
                            type="button"
                            onclick={copyInstallCommand}
                        >
                            {copied ? "Copied" : "Copy"}
                        </button>
                    </div>

                    <div class="pill-tabs compact-tabs">
                        {#each Object.entries(installs) as [key, entry]}
                            <button
                                type="button"
                                class:active-pill={installTab === key}
                                onclick={() => (installTab = key as InstallTab)}
                            >
                                {entry.label}
                            </button>
                        {/each}
                    </div>

                    <div class="command-box">
                        <code>{installs[installTab].command}</code>
                    </div>

                    <p class="panel-note">{installs[installTab].note}</p>
                    <p class="panel-detail">{installs[installTab].detail}</p>
                </section>

                <section class="mode-stage surface-panel">
                    <div class="panel-head">
                        <div>
                            <p class="section-kicker">Live stage</p>
                            <h2>One flow. Three authoring paths.</h2>
                        </div>
                        <p class="mini-proof">Same renderer, same terminal model.</p>
                    </div>

                    <div class="pill-tabs">
                        {#each Object.entries(showcases) as [key, entry]}
                            <button
                                type="button"
                                class:active-pill={showcaseTab === key}
                                onclick={() =>
                                    (showcaseTab = key as ShowcaseTab)}
                            >
                                {entry.label}
                            </button>
                        {/each}
                    </div>

                    <div class="stage-window">
                        <div class="stage-rail">
                            <div class="rail-step active-step">
                                <span>01</span>
                                <div>
                                    <strong>Author</strong>
                                    <p>{showcases[showcaseTab].label}</p>
                                </div>
                            </div>
                            <div class="rail-step">
                                <span>02</span>
                                <div>
                                    <strong>Preview</strong>
                                    <p>Shared runtime stage</p>
                                </div>
                            </div>
                            <div class="rail-step">
                                <span>03</span>
                                <div>
                                    <strong>Ship</strong>
                                    <p>CLI or creator export</p>
                                </div>
                            </div>
                        </div>

                        <div class="stage-panels">
                            <div class="code-surface">
                                <div class="surface-label">
                                    {showcases[showcaseTab].label}
                                </div>
                                <pre>{showcases[showcaseTab].code}</pre>
                            </div>

                            <div class="terminal-surface">
                                <div class="terminal-head">
                                    <span class="terminal-dot"></span>
                                    <span class="terminal-dot"></span>
                                    <span class="terminal-dot"></span>
                                    <p>{showcases[showcaseTab].terminalTitle}</p>
                                </div>

                                <div class="terminal-body">
                                    {#each showcases[showcaseTab].terminalLines as line, index}
                                        <div class:terminal-accent={index === 0}>
                                            {line}
                                        </div>
                                    {/each}
                                    <span class="terminal-cursor"></span>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="chip-row">
                        {#each showcases[showcaseTab].chips as chip}
                            <span>{chip}</span>
                        {/each}
                    </div>

                    <p class="panel-detail">{showcases[showcaseTab].caption}</p>
                </section>
            </div>
        </div>

        <section class="stats-dock">
            <div>
                <strong>2,297+</strong>
                <span>Flows built</span>
            </div>
            <div>
                <strong>28</strong>
                <span>Widget types</span>
            </div>
            <div>
                <strong>47ms</strong>
                <span>Avg render</span>
            </div>
            <div>
                <strong>98%</strong>
                <span>Preview parity</span>
            </div>
        </section>
    </section>

    <section class="section-shell stage-grid slabs-section">
        <div class="section-heading">
            <p>Core principles</p>
            <h2>
                Built on three
                <span>engineering slabs.</span>
            </h2>
        </div>

        <div class="slabs-grid">
            {#each slabs as slab}
                <article class="slab-card">
                    <div class="slab-topline">
                        <p class="section-kicker">{slab.eyebrow}</p>
                        <div class="slab-stat">
                            <strong>{slab.stat}</strong>
                            <span>{slab.statLabel}</span>
                        </div>
                    </div>

                    <h3>{slab.title}</h3>
                    <p class="slab-body">{slab.body}</p>

                    <div class="slab-points">
                        {#each slab.points as point}
                            <div>{point}</div>
                        {/each}
                    </div>
                </article>
            {/each}
        </div>
    </section>

    <section class="section-shell stage-grid equivalence-section">
        <div class="section-heading">
            <p>Same flow, three entry points</p>
            <h2>
                Choose the authoring surface.
                <span>Keep the runtime intact.</span>
            </h2>
        </div>

        <div class="entry-grid">
            {#each entryPoints as entry}
                <article class="entry-card">
                    <p class="section-kicker">{entry.title}</p>
                    <h3>{entry.title}</h3>
                    <p>{entry.body}</p>
                    <div class="entry-snippet">
                        <pre>{entry.snippet}</pre>
                    </div>
                    <div class="entry-footer">{entry.footer}</div>
                </article>
            {/each}
        </div>

        <div class="equivalence-runway">
            <div class="runway-label">Converges into one terminal interaction model</div>
            <div class="runway-rule"></div>
            <div class="runway-terminal">
                <div class="terminal-head">
                    <span class="terminal-dot"></span>
                    <span class="terminal-dot"></span>
                    <span class="terminal-dot"></span>
                    <p>same runtime result</p>
                </div>
                <div class="terminal-body compact-body">
                    <div class="terminal-accent">Basic user data</div>
                    <div>Name: Ada Lovelace</div>
                    <div>Email: ada@analytical.engine</div>
                    <div>Project directory: ~/workspace/steply</div>
                    <span class="terminal-cursor"></span>
                </div>
            </div>
        </div>
    </section>

    <section class="section-shell stage-grid surfaces-section">
        <div class="widget-wall">
            <div class="section-heading narrow-heading">
                <p>Widget surface</p>
                <h2>
                    Not a prompt picker.
                    <span>A terminal interface system.</span>
                </h2>
            </div>

            <div class="widget-mosaic">
                {#each widgetTiles as tile}
                    <article
                        class:tile-wide={tile.span === "wide"}
                        class:tile-tall={tile.span === "tall"}
                        class="widget-tile"
                    >
                        <p class="section-kicker">Widget</p>
                        <h3>{tile.name}</h3>
                        <p>{tile.detail}</p>
                    </article>
                {/each}
            </div>
        </div>

        <aside class="runtime-aside">
            <div class="surface-panel runtime-card">
                <p class="section-kicker">Runtime surface</p>
                <h3>What makes the interaction model feel real.</h3>
                <div class="runtime-list">
                    <div>
                        <strong>Steps with focus flow</strong>
                        <span>Multi-widget screens, submit semantics, and proper navigation.</span>
                    </div>
                    <div>
                        <strong>Validation on transition</strong>
                        <span>Enter, step submit, and task triggers behave like one system.</span>
                    </div>
                    <div>
                        <strong>Tasks and state</strong>
                        <span>Store values, react to runtime events, and wire flows to real command execution.</span>
                    </div>
                    <div>
                        <strong>Docs and CLI generated from schema</strong>
                        <span>The model layer stays central instead of drifting into side tables.</span>
                    </div>
                </div>
            </div>
        </aside>
    </section>

    <section class="section-shell stage-grid assembly-section" id="assembly-line">
        <div class="section-heading">
            <p>Assembly line</p>
            <h2>
                From contract to
                <span>running terminal flow.</span>
            </h2>
        </div>

        <div class="assembly-grid">
            {#each workflow as step}
                <article class="assembly-item">
                    <div class="assembly-index">{step.index}</div>
                    <div class="assembly-copy">
                        <h3>{step.title}</h3>
                        <p>{step.body}</p>
                        <span>{step.artifact}</span>
                    </div>
                </article>
            {/each}
        </div>
    </section>

    <section class="section-shell stage-grid examples-section">
        <div class="section-heading">
            <p>Flows in the wild</p>
            <h2>
                Concrete terminal products.
                <span>Not abstract feature lists.</span>
            </h2>
        </div>

        <div class="examples-layout">
            <article class="example-feature">
                <div class="example-meta">
                    <p class="section-kicker">{examples[0].accent}</p>
                    <div class="chip-row">
                        {#each examples[0].tags as tag}
                            <span>{tag}</span>
                        {/each}
                    </div>
                </div>
                <h3>{examples[0].title}</h3>
                <p>{examples[0].body}</p>
                <div class="example-split">
                    <div class="entry-snippet">
                        <pre>steps:
  - id: bootstrap
    widgets:
      - type: text_input
      - type: file_browser
      - type: command_runner</pre>
                    </div>
                    <div class="mini-terminal">
                        <div class="terminal-head">
                            <span class="terminal-dot"></span>
                            <span class="terminal-dot"></span>
                            <span class="terminal-dot"></span>
                            <p>bootstrap wizard</p>
                        </div>
                        <div class="terminal-body compact-body">
                            <div class="terminal-accent">Project setup</div>
                            <div>Name: steply</div>
                            <div>Directory: ~/workspace/steply</div>
                            <div>Template: rust-cli</div>
                        </div>
                    </div>
                </div>
            </article>

            <div class="example-stack">
                {#each examples.slice(1) as example}
                    <article class="example-sidecard">
                        <p class="section-kicker">{example.accent}</p>
                        <h3>{example.title}</h3>
                        <p>{example.body}</p>
                        <div class="chip-row">
                            {#each example.tags as tag}
                                <span>{tag}</span>
                            {/each}
                        </div>
                    </article>
                {/each}
            </div>
        </div>
    </section>

    <section class="section-shell stage-grid final-dock-section">
        <div class="final-dock">
            <div class="final-copy">
                <p class="section-kicker">Get started</p>
                <h2>
                    Install the CLI.
                    <span>Or start in the creator.</span>
                </h2>
                <p>
                    Use the same model to author flows, preview them in the
                    browser, and ship them to the terminal when they are ready.
                </p>
            </div>

            <div class="final-actions">
                <a class="primary-action" href="/creator">Open Creator ↗</a>
                <a class="secondary-action" href="/docs">Read the docs</a>
            </div>

            <div class="final-command-grid">
                {#each Object.values(installs) as entry}
                    <div class="final-command">
                        <strong>{entry.label}</strong>
                        <code>{entry.command}</code>
                    </div>
                {/each}
            </div>
        </div>
    </section>

    <footer class="site-footer stage-grid">
        <a class="brand footer-brand" href="/">
            <span class="brand-mark">s</span>
            <span class="brand-name">Steply</span>
            <span class="footer-version">· v0.9.2</span>
        </a>

        <nav class="footer-nav">
            <a href="https://github.com" rel="noreferrer">GitHub</a>
            <a href="/docs">Docs</a>
            <a href="/changelog">Changelog</a>
            <a href="/license">License</a>
        </nav>
    </footer>
</div>

<style>
    :global(html) {
        scroll-behavior: smooth;
    }

    :global(body) {
        margin: 0;
        background:
            radial-gradient(circle at 14% 18%, rgba(255, 198, 74, 0.09), transparent 24%),
            radial-gradient(circle at 78% 68%, rgba(255, 255, 255, 0.035), transparent 28%),
            linear-gradient(180deg, #171b22 0%, #12161d 100%);
        color: #f2efe8;
        font-family:
            "Avenir Next", "Segoe UI", "Helvetica Neue", Helvetica, Arial,
            sans-serif;
    }

    :global(*) {
        box-sizing: border-box;
    }

    .page-shell {
        position: relative;
        overflow: clip;
        background:
            radial-gradient(circle at 50% 24%, rgba(255, 255, 255, 0.03), transparent 18%),
            radial-gradient(circle at 80% 72%, rgba(255, 190, 34, 0.04), transparent 22%);
    }

    .stage-grid,
    .site-footer {
        position: relative;
        padding-inline: clamp(1.2rem, 2.2vw, 2rem);
    }

    .stage-grid::before,
    .site-footer::before {
        content: "";
        position: absolute;
        inset: 0;
        pointer-events: none;
        background:
            linear-gradient(rgba(255, 255, 255, 0.035) 1px, transparent 1px),
            linear-gradient(90deg, rgba(255, 255, 255, 0.028) 1px, transparent 1px);
        background-size: 52px 52px;
        opacity: 0.28;
    }

    .stage-grid::after,
    .site-footer::after {
        content: "";
        position: absolute;
        inset: 0;
        pointer-events: none;
        background:
            radial-gradient(circle at 16% 18%, rgba(18, 22, 29, 0.26), transparent 34%),
            radial-gradient(circle at 82% 38%, rgba(18, 22, 29, 0.18), transparent 36%),
            linear-gradient(180deg, rgba(18, 22, 29, 0.18) 0%, rgba(18, 22, 29, 0.03) 18%, rgba(18, 22, 29, 0.03) 82%, rgba(18, 22, 29, 0.16) 100%);
    }

    .noise {
        position: absolute;
        inset: 0;
        pointer-events: none;
        background:
            repeating-linear-gradient(
                0deg,
                rgba(255, 255, 255, 0.012) 0,
                rgba(255, 255, 255, 0.012) 1px,
                transparent 2px,
                transparent 4px
            ),
            repeating-linear-gradient(
                90deg,
                transparent 0,
                transparent 10px,
                rgba(255, 255, 255, 0.008) 11px,
                transparent 12px
            );
        mix-blend-mode: soft-light;
        opacity: 0.34;
    }

    .hero-stage {
        min-height: 100vh;
        border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    }

    .hero-halo {
        position: absolute;
        border-radius: 999px;
        filter: blur(78px);
        pointer-events: none;
    }

    .hero-halo-left {
        left: -8rem;
        top: 14rem;
        width: 22rem;
        height: 22rem;
        background: rgba(248, 190, 34, 0.12);
    }

    .hero-halo-right {
        right: 8%;
        top: 8rem;
        width: 18rem;
        height: 18rem;
        background: rgba(255, 255, 255, 0.04);
    }

    .topbar {
        position: sticky;
        top: 0;
        z-index: 20;
        display: grid;
        grid-template-columns: 1fr auto 1fr;
        align-items: center;
        gap: 1rem;
        min-height: 4.5rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(15, 18, 24, 0.78);
        backdrop-filter: blur(16px);
    }

    .brand {
        display: inline-flex;
        align-items: center;
        gap: 0.85rem;
        color: #f2efe8;
        text-decoration: none;
        font-weight: 700;
    }

    .brand-mark {
        display: inline-grid;
        place-items: center;
        width: 1.9rem;
        height: 1.9rem;
        border-radius: 0.3rem;
        background: #f8be22;
        color: #151922;
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 1rem;
        text-transform: lowercase;
    }

    .brand-name {
        font-size: 1.5rem;
        letter-spacing: -0.04em;
    }

    .topnav {
        display: inline-flex;
        justify-content: center;
        gap: 2rem;
    }

    .topnav a,
    .footer-nav a {
        color: rgba(242, 239, 232, 0.62);
        text-decoration: none;
        font-size: 1rem;
    }

    .topbar-cta,
    .primary-action {
        justify-self: end;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        min-height: 3rem;
        padding: 0 1.35rem;
        border: 1px solid rgba(255, 190, 34, 0.42);
        background: #f8be22;
        color: #171b23;
        text-decoration: none;
        font-size: 1rem;
        font-weight: 700;
        letter-spacing: -0.02em;
        box-shadow: 0 0 36px rgba(248, 190, 34, 0.16);
        transition:
            transform 160ms ease,
            box-shadow 160ms ease;
    }

    .topbar-cta:hover,
    .primary-action:hover,
    .secondary-action:hover {
        transform: translateY(-1px);
    }

    .secondary-action {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        min-height: 3rem;
        padding: 0 1.35rem;
        border: 1px solid rgba(255, 255, 255, 0.1);
        background: rgba(20, 24, 32, 0.72);
        color: rgba(242, 239, 232, 0.8);
        text-decoration: none;
        font-weight: 600;
    }

    .hero-inner,
    .section-shell,
    .site-footer {
        width: min(100%, 92rem);
        margin: 0 auto;
        position: relative;
        z-index: 1;
    }

    .hero-inner {
        display: grid;
        grid-template-columns: minmax(0, 1fr) minmax(24rem, 0.98fr);
        gap: clamp(2rem, 4vw, 4rem);
        padding-block: clamp(3rem, 8vw, 6.6rem) 4rem;
    }

    .hero-copy {
        padding-top: clamp(2.8rem, 8vw, 6.4rem);
    }

    .eyebrow-badge,
    .section-kicker,
    .section-heading p {
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.86rem;
        letter-spacing: 0.12em;
        text-transform: uppercase;
    }

    .eyebrow-badge {
        display: inline-flex;
        align-items: center;
        min-height: 2rem;
        padding: 0 1rem;
        border: 1px solid rgba(255, 255, 255, 0.08);
        background: rgba(19, 23, 30, 0.8);
        color: rgba(212, 216, 224, 0.74);
        text-transform: none;
        letter-spacing: 0.02em;
    }

    .section-kicker,
    .section-heading p {
        color: rgba(248, 190, 34, 0.74);
    }

    h1,
    h2,
    h3,
    p,
    pre {
        margin: 0;
    }

    h1 {
        max-width: 9.5ch;
        margin-top: 2.4rem;
        font-size: clamp(4.4rem, 8vw, 7rem);
        line-height: 0.9;
        letter-spacing: -0.1em;
        font-weight: 800;
    }

    h1 span,
    .section-heading span,
    .final-copy h2 span {
        display: block;
        color: #f8be22;
    }

    .hero-description {
        max-width: 34rem;
        margin-top: 2rem;
        color: rgba(232, 228, 219, 0.7);
        font-size: clamp(1.24rem, 1.9vw, 1.56rem);
        line-height: 1.55;
    }

    .hero-actions {
        display: flex;
        flex-wrap: wrap;
        gap: 1rem;
        margin-top: 2rem;
    }

    .hero-signal-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 0.9rem;
        max-width: 40rem;
        margin-top: 2rem;
    }

    .hero-signal-grid div,
    .final-command,
    .entry-card,
    .example-sidecard {
        border: 1px solid rgba(255, 255, 255, 0.08);
        background: rgba(18, 22, 29, 0.76);
    }

    .hero-signal-grid div {
        padding: 1rem 1rem 1.05rem;
    }

    .hero-signal-grid strong,
    .runtime-list strong,
    .final-command strong {
        display: block;
        font-size: 0.96rem;
        letter-spacing: -0.02em;
    }

    .hero-signal-grid span,
    .runtime-list span {
        display: block;
        margin-top: 0.35rem;
        color: rgba(229, 225, 217, 0.62);
        line-height: 1.45;
    }

    .hero-foot {
        display: flex;
        flex-wrap: wrap;
        gap: 1rem;
        align-items: center;
        margin-top: 2rem;
        color: rgba(203, 197, 186, 0.46);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.95rem;
    }

    .divider {
        width: 3rem;
        height: 1px;
        background: rgba(255, 255, 255, 0.16);
    }

    .hero-stage-stack {
        display: grid;
        gap: 1.2rem;
    }

    .surface-panel,
    .slab-card,
    .widget-tile,
    .assembly-item,
    .example-feature,
    .final-dock {
        position: relative;
        border: 1px solid rgba(255, 255, 255, 0.08);
        background:
            linear-gradient(180deg, rgba(22, 27, 35, 0.92), rgba(17, 21, 28, 0.88));
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.02);
    }

    .surface-panel::before,
    .slab-card::before,
    .widget-tile::before,
    .assembly-item::before,
    .example-feature::before,
    .example-sidecard::before,
    .final-dock::before {
        content: "";
        position: absolute;
        inset: 0;
        pointer-events: none;
        background:
            linear-gradient(180deg, rgba(255, 255, 255, 0.015), transparent 18%),
            radial-gradient(circle at 12% 0%, rgba(255, 190, 34, 0.05), transparent 22%);
    }

    .panel-head {
        display: flex;
        justify-content: space-between;
        gap: 1rem;
        align-items: start;
    }

    .panel-head h2,
    .section-heading h2,
    .final-copy h2 {
        font-size: clamp(2.5rem, 5vw, 4.2rem);
        line-height: 0.96;
        letter-spacing: -0.08em;
        max-width: 10ch;
    }

    .panel-head h2 {
        font-size: clamp(1.8rem, 3vw, 2.5rem);
        max-width: none;
    }

    .copy-button {
        border: 1px solid rgba(255, 255, 255, 0.08);
        background: rgba(18, 22, 29, 0.72);
        color: rgba(242, 239, 232, 0.8);
        min-height: 2.6rem;
        padding: 0 1rem;
        font-weight: 700;
    }

    .pill-tabs {
        display: inline-flex;
        flex-wrap: wrap;
        gap: 0.55rem;
    }

    .compact-tabs {
        margin-top: 1rem;
    }

    .pill-tabs button {
        border: 1px solid rgba(255, 255, 255, 0.08);
        background: rgba(18, 22, 29, 0.62);
        color: rgba(230, 226, 217, 0.54);
        min-height: 2.2rem;
        padding: 0 0.85rem;
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.84rem;
        letter-spacing: 0.1em;
        text-transform: uppercase;
    }

    .pill-tabs .active-pill {
        border-color: rgba(248, 190, 34, 0.42);
        color: rgba(248, 190, 34, 0.9);
        background: rgba(248, 190, 34, 0.08);
    }

    .install-dock,
    .mode-stage,
    .runtime-card {
        padding: 1.3rem;
    }

    .command-box,
    .entry-snippet,
    .code-surface,
    .terminal-surface,
    .mini-terminal {
        border: 1px solid rgba(255, 255, 255, 0.07);
        background: rgba(9, 13, 20, 0.86);
    }

    .command-box {
        margin-top: 1rem;
        padding: 1rem;
        color: #f8be22;
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        overflow: auto;
    }

    .panel-note,
    .panel-detail,
    .entry-card p,
    .slab-body,
    .assembly-copy p,
    .example-feature p,
    .example-sidecard p,
    .final-copy p {
        color: rgba(229, 225, 217, 0.64);
        line-height: 1.55;
    }

    .panel-note {
        margin-top: 0.8rem;
    }

    .panel-detail {
        margin-top: 0.45rem;
    }

    .mini-proof {
        max-width: 13rem;
        color: rgba(229, 225, 217, 0.54);
        line-height: 1.45;
        text-align: right;
    }

    .stage-window {
        display: grid;
        grid-template-columns: 12rem minmax(0, 1fr);
        gap: 1rem;
        margin-top: 1rem;
    }

    .stage-rail {
        display: grid;
        gap: 0.7rem;
        align-content: start;
    }

    .rail-step {
        display: grid;
        grid-template-columns: 2.4rem 1fr;
        gap: 0.8rem;
        padding: 0.85rem;
        border: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(18, 22, 29, 0.58);
        color: rgba(229, 225, 217, 0.56);
    }

    .rail-step span,
    .assembly-index {
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        color: rgba(248, 190, 34, 0.78);
    }

    .rail-step strong {
        display: block;
        color: #f2efe8;
    }

    .rail-step p {
        margin-top: 0.15rem;
        color: rgba(229, 225, 217, 0.52);
        font-size: 0.92rem;
    }

    .active-step {
        border-color: rgba(248, 190, 34, 0.22);
        background: rgba(248, 190, 34, 0.06);
    }

    .stage-panels {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 1rem;
    }

    .code-surface,
    .terminal-surface,
    .mini-terminal,
    .entry-snippet {
        overflow: hidden;
    }

    .code-surface pre,
    .entry-snippet pre {
        padding: 1rem;
        color: rgba(227, 223, 214, 0.74);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.98rem;
        line-height: 1.5;
        white-space: pre-wrap;
    }

    .surface-label {
        padding: 0.8rem 1rem 0;
        color: rgba(248, 190, 34, 0.8);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.8rem;
        letter-spacing: 0.12em;
        text-transform: uppercase;
    }

    .terminal-head {
        display: flex;
        align-items: center;
        gap: 0.45rem;
        min-height: 2.2rem;
        padding: 0 0.9rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        color: rgba(210, 205, 196, 0.5);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.84rem;
    }

    .terminal-dot {
        width: 0.42rem;
        height: 0.42rem;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.12);
    }

    .terminal-body {
        position: relative;
        display: grid;
        gap: 0.35rem;
        min-height: 18rem;
        padding: 1rem;
        color: rgba(219, 215, 207, 0.76);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 1rem;
        line-height: 1.45;
    }

    .compact-body {
        min-height: auto;
    }

    .terminal-accent {
        color: rgba(248, 190, 34, 0.86);
    }

    .terminal-cursor {
        width: 0.55rem;
        height: 1rem;
        background: rgba(248, 190, 34, 0.82);
        margin-top: 0.25rem;
    }

    .chip-row {
        display: flex;
        flex-wrap: wrap;
        gap: 0.55rem;
        margin-top: 1rem;
    }

    .chip-row span,
    .entry-footer {
        border: 1px solid rgba(255, 255, 255, 0.08);
        background: rgba(18, 22, 29, 0.58);
        color: rgba(226, 221, 213, 0.65);
        padding: 0.45rem 0.7rem;
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.78rem;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .stats-dock {
        display: grid;
        grid-template-columns: repeat(4, minmax(0, 1fr));
        gap: 1px;
        width: min(100%, 92rem);
        margin: 0 auto;
        border: 1px solid rgba(255, 255, 255, 0.07);
        border-bottom: 0;
        background: rgba(255, 255, 255, 0.07);
        position: relative;
        z-index: 1;
    }

    .stats-dock div {
        padding: 1.75rem 1.2rem;
        background: rgba(18, 22, 29, 0.82);
    }

    .stats-dock strong {
        display: block;
        font-size: clamp(2.2rem, 4vw, 3.2rem);
        letter-spacing: -0.06em;
    }

    .stats-dock span {
        display: block;
        margin-top: 0.55rem;
        color: rgba(205, 198, 187, 0.46);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.84rem;
        letter-spacing: 0.14em;
        text-transform: uppercase;
    }

    .section-shell {
        padding-block: clamp(5rem, 10vw, 8.5rem);
    }

    .section-heading {
        display: grid;
        gap: 0.85rem;
        max-width: 40rem;
        margin-bottom: 2rem;
        position: relative;
        z-index: 1;
    }

    .narrow-heading {
        max-width: 32rem;
    }

    .slabs-grid,
    .entry-grid {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        gap: 1.2rem;
        position: relative;
        z-index: 1;
    }

    .slab-card {
        padding: 1.35rem;
    }

    .slab-topline {
        display: flex;
        justify-content: space-between;
        gap: 1rem;
        align-items: start;
    }

    .slab-stat {
        text-align: right;
    }

    .slab-stat strong {
        display: block;
        font-size: 2rem;
        letter-spacing: -0.06em;
    }

    .slab-stat span {
        color: rgba(205, 198, 187, 0.46);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.78rem;
        letter-spacing: 0.12em;
        text-transform: uppercase;
    }

    .slab-card h3,
    .entry-card h3,
    .widget-tile h3,
    .assembly-copy h3,
    .example-feature h3,
    .example-sidecard h3,
    .runtime-card h3 {
        margin-top: 1rem;
        font-size: clamp(1.6rem, 2.2vw, 2rem);
        line-height: 1.05;
        letter-spacing: -0.05em;
    }

    .slab-body {
        margin-top: 0.8rem;
    }

    .slab-points {
        display: grid;
        gap: 0.55rem;
        margin-top: 1.2rem;
    }

    .slab-points div {
        padding-top: 0.75rem;
        border-top: 1px solid rgba(255, 255, 255, 0.08);
        color: rgba(233, 228, 220, 0.72);
    }

    .entry-card {
        padding: 1.25rem;
    }

    .entry-snippet {
        margin-top: 1rem;
    }

    .entry-footer {
        margin-top: 1rem;
        width: fit-content;
    }

    .equivalence-runway {
        margin-top: 1.3rem;
        position: relative;
        z-index: 1;
    }

    .runway-label {
        color: rgba(228, 223, 214, 0.7);
        margin-bottom: 0.7rem;
    }

    .runway-rule {
        height: 1px;
        background:
            linear-gradient(90deg, rgba(255, 255, 255, 0.08), rgba(248, 190, 34, 0.26), rgba(255, 255, 255, 0.08));
        margin-bottom: 1rem;
    }

    .runway-terminal {
        width: min(100%, 40rem);
    }

    .surfaces-section {
        display: grid;
        grid-template-columns: minmax(0, 1.4fr) minmax(20rem, 0.82fr);
        gap: 1.4rem;
        align-items: start;
    }

    .widget-mosaic {
        display: grid;
        grid-template-columns: repeat(4, minmax(0, 1fr));
        gap: 1rem;
        position: relative;
        z-index: 1;
    }

    .widget-tile {
        padding: 1.15rem;
        min-height: 12rem;
    }

    .widget-tile p:last-child {
        margin-top: 0.7rem;
        color: rgba(229, 225, 217, 0.62);
        line-height: 1.5;
    }

    .tile-wide {
        grid-column: span 2;
    }

    .tile-tall {
        grid-row: span 2;
        min-height: 25rem;
    }

    .runtime-card {
        position: sticky;
        top: 6rem;
    }

    .runtime-list {
        display: grid;
        gap: 1rem;
        margin-top: 1rem;
    }

    .runtime-list div {
        padding-top: 0.95rem;
        border-top: 1px solid rgba(255, 255, 255, 0.08);
    }

    .assembly-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 1rem 1.2rem;
        position: relative;
        z-index: 1;
    }

    .assembly-item {
        display: grid;
        grid-template-columns: 4.2rem 1fr;
        gap: 1rem;
        padding: 1.15rem;
    }

    .assembly-index {
        font-size: 1.05rem;
    }

    .assembly-copy span {
        display: inline-flex;
        margin-top: 0.9rem;
        color: rgba(248, 190, 34, 0.82);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.8rem;
        letter-spacing: 0.1em;
        text-transform: uppercase;
    }

    .examples-layout {
        display: grid;
        grid-template-columns: minmax(0, 1.2fr) minmax(18rem, 0.8fr);
        gap: 1.2rem;
        position: relative;
        z-index: 1;
    }

    .example-feature,
    .example-sidecard {
        padding: 1.3rem;
    }

    .example-meta {
        display: flex;
        justify-content: space-between;
        gap: 1rem;
        align-items: start;
    }

    .example-split {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 1rem;
        margin-top: 1rem;
    }

    .example-stack {
        display: grid;
        gap: 1rem;
    }

    .final-dock {
        padding: 1.5rem;
        position: relative;
        z-index: 1;
    }

    .final-dock-section {
        padding-top: 4rem;
    }

    .final-dock {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        gap: 1.4rem;
        align-items: start;
    }

    .final-actions {
        display: flex;
        gap: 1rem;
        align-self: center;
    }

    .final-command-grid {
        grid-column: 1 / -1;
        display: grid;
        grid-template-columns: repeat(4, minmax(0, 1fr));
        gap: 1rem;
    }

    .final-command {
        padding: 1rem;
    }

    .final-command code {
        display: block;
        margin-top: 0.6rem;
        color: rgba(248, 190, 34, 0.88);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.88rem;
        line-height: 1.45;
        word-break: break-word;
    }

    .site-footer {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 1rem;
        width: min(100%, 92rem);
        margin: 0 auto;
        min-height: 6rem;
        border-top: 1px solid rgba(255, 255, 255, 0.07);
        position: relative;
        z-index: 1;
    }

    .footer-version {
        color: rgba(205, 198, 187, 0.42);
    }

    .footer-nav {
        display: inline-flex;
        gap: 1.5rem;
    }

    @media (max-width: 1100px) {
        .hero-inner,
        .surfaces-section,
        .examples-layout,
        .final-dock {
            grid-template-columns: 1fr;
        }

        .stage-window,
        .stage-panels,
        .slabs-grid,
        .entry-grid,
        .assembly-grid,
        .final-command-grid {
            grid-template-columns: 1fr;
        }

        .widget-mosaic {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }

        .tile-wide,
        .tile-tall {
            grid-column: auto;
            grid-row: auto;
            min-height: 12rem;
        }

        .runtime-card {
            position: static;
        }
    }

    @media (max-width: 820px) {
        .topbar {
            grid-template-columns: 1fr auto;
            padding-block: 0.75rem;
        }

        .topnav {
            display: none;
        }

        .hero-signal-grid,
        .stats-dock,
        .widget-mosaic,
        .example-split {
            grid-template-columns: 1fr;
        }

        .stats-dock {
            border-bottom: 1px solid rgba(255, 255, 255, 0.07);
        }

        .stats-dock div {
            padding: 1.3rem 1rem;
        }

        .site-footer {
            flex-direction: column;
            align-items: start;
            padding-block: 1.5rem;
        }
    }
</style>
