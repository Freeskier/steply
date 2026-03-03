<script lang="ts">
    let { data } = $props();

    type RenderColor = string | { rgb: [number, number, number] } | null;
    type RenderStyle = {
        background: RenderColor;
        bold: boolean;
        color: RenderColor;
        strike: "inherit" | "on" | "off";
    };
    type RenderSpan = {
        style: RenderStyle;
        text: string;
        wrap_mode: "wrap" | "no_wrap";
    };
    type RenderDoc = {
        lines: RenderSpan[][];
        cursor: { col: number; row: number } | null;
        cursor_visible: boolean;
        terminal: { width: number; height: number };
    };

    type PreviewRequest = {
        scope: "current" | "flow" | "step" | "widget";
        active_step_id?: string;
        step_id?: string;
        widget_id?: string;
        width?: number;
        height?: number;
    };

    let hydrated = $state(false);
    let yamlText = $state("");
    let renderRequest = $state<PreviewRequest>({
        scope: "flow",
        width: 100,
        height: 40,
    });
    let rendered = $state<RenderDoc | null>(null);
    let errorText = $state<string | null>(null);
    let isRendering = $state(false);
    let debounceTimer: ReturnType<typeof setTimeout> | null = null;

    $effect(() => {
        if (hydrated) return;
        yamlText = data.yaml as string;
        renderRequest = data.request as PreviewRequest;
        rendered = (data.rendered as RenderDoc | null) ?? null;
        errorText = (data.error as string | null) ?? null;
        hydrated = true;
    });

    function terminalWidth(): number {
        return rendered?.terminal?.width ?? renderRequest.width ?? 100;
    }

    function terminalHeight(): number {
        return rendered?.terminal?.height ?? renderRequest.height ?? 40;
    }

    function colorToCss(color: RenderColor): string {
        if (!color) return "inherit";
        if (typeof color === "object" && "rgb" in color) {
            return `rgb(${color.rgb[0]}, ${color.rgb[1]}, ${color.rgb[2]})`;
        }
        switch (color) {
            case "reset":
                return "inherit";
            case "black":
                return "#0f172a";
            case "dark_grey":
                return "#546079";
            case "red":
                return "#ff6f91";
            case "green":
                return "#a6e3a1";
            case "yellow":
                return "#f9e2af";
            case "blue":
                return "#89b4fa";
            case "magenta":
                return "#cba6f7";
            case "cyan":
                return "#7dd3fc";
            case "white":
                return "#e5e7eb";
            default:
                return "inherit";
        }
    }

    function spanStyle(span: RenderSpan): string {
        const style = span.style;
        const rules = [
            `color: ${colorToCss(style.color)}`,
            `font-weight: ${style.bold ? 700 : 400}`,
            style.strike === "on"
                ? "text-decoration: line-through"
                : "text-decoration: none",
        ];
        if (style.background && style.background !== "reset") {
            rules.push(`background: ${colorToCss(style.background)}`);
        }
        return rules.join(";");
    }

    function sanitizeTerminalText(input: string): string {
        return input
            .replace(/\u001b\][^\u0007]*(?:\u0007|\u001b\\)/g, "")
            .replace(/\u001b\[[0-9;?]*[ -/]*[@-~]/g, "");
    }

    async function renderNow() {
        isRendering = true;
        try {
            const response = await fetch("/api/preview", {
                method: "POST",
                headers: { "content-type": "application/json" },
                body: JSON.stringify({
                    yaml: yamlText,
                    request: renderRequest,
                }),
            });
            const payload = await response.json();
            if (!response.ok || !payload?.ok) {
                rendered = null;
                errorText =
                    payload?.error ?? `Render failed (${response.status})`;
                return;
            }
            rendered = payload.rendered as RenderDoc;
            errorText = null;
        } catch (error) {
            rendered = null;
            errorText = error instanceof Error ? error.message : String(error);
        } finally {
            isRendering = false;
        }
    }

    function scheduleRender() {
        if (debounceTimer) clearTimeout(debounceTimer);
        debounceTimer = setTimeout(() => {
            void renderNow();
        }, 250);
    }
</script>

<svelte:head>
    <title>Steply WASM SSR Demo</title>
</svelte:head>

<main>
    <div class="headline">
        <h1>Steply SSR Terminal Preview</h1>
        <p>
            Edytuj YAML po lewej. Podglad terminala po prawej jest renderowany
            na serwerze przez
            <code>steply-wasm</code>.
        </p>
    </div>

    <section class="workspace">
        <div class="editor-pane">
            <div class="pane-header">
                <h2>flow.yaml</h2>
                <button
                    type="button"
                    onclick={() => void renderNow()}
                    disabled={isRendering}
                >
                    {isRendering ? "Rendering..." : "Render now"}
                </button>
            </div>
            <textarea
                bind:value={yamlText}
                oninput={scheduleRender}
                spellcheck="false"
            ></textarea>
        </div>

        <div class="terminal-shell">
            <div class="terminal-header">
                <div class="lights">
                    <span class="light red"></span>
                    <span class="light yellow"></span>
                    <span class="light green"></span>
                </div>
                <div class="title">steply-preview://flow</div>
                <div class="meta">{terminalWidth()}x{terminalHeight()}</div>
            </div>

            {#if errorText}
                <div class="terminal-body">
                    <pre class="error">{errorText}</pre>
                </div>
            {:else if rendered}
                <div class="terminal-body">
                    <div
                        class="terminal-grid"
                        style={`--cols:${terminalWidth()}; --rows:${terminalHeight()}; --cursor-col:${rendered.cursor?.col ?? 0}; --cursor-row:${rendered.cursor?.row ?? 0};`}
                    >
                        {#each rendered.lines ?? [] as line}
                            <div class="row">
                                {#each line as span}
                                    <span style={spanStyle(span)}
                                        >{sanitizeTerminalText(span.text)}</span
                                    >
                                {/each}
                            </div>
                        {/each}

                        {#if rendered.cursor_visible && rendered.cursor}
                            <div class="cursor"></div>
                        {/if}
                    </div>
                </div>
            {/if}
        </div>
    </section>
</main>

<style>
    :global(body) {
        margin: 0;
        color: #dbe5f5;
        background:
            radial-gradient(
                1200px 500px at 20% 0%,
                #1b2f5f55 0%,
                transparent 70%
            ),
            radial-gradient(
                1000px 600px at 90% 100%,
                #0f3b7d40 0%,
                transparent 70%
            ),
            linear-gradient(180deg, #05080f 0%, #070d18 100%);
        font-family: "JetBrains Mono", "Fira Code", "Cascadia Mono", monospace;
        min-height: 100vh;
    }

    main {
        max-width: 1440px;
        margin: 0 auto;
        padding: 20px;
    }

    .headline h1 {
        margin: 0 0 8px;
        font-size: 24px;
        letter-spacing: 0.01em;
    }

    .headline p {
        margin: 0 0 16px;
        color: #9cb0cf;
        max-width: 980px;
        line-height: 1.45;
    }

    .headline code {
        color: #9fd2ff;
    }

    .workspace {
        display: grid;
        grid-template-columns: minmax(320px, 42%) minmax(520px, 58%);
        gap: 14px;
        align-items: stretch;
    }

    .editor-pane,
    .terminal-shell {
        border-radius: 14px;
        border: 1px solid #27436e;
        background: #060b14;
        box-shadow:
            0 20px 80px rgba(1, 6, 20, 0.9),
            inset 0 0 0 1px rgba(113, 170, 240, 0.1);
        overflow: hidden;
        min-height: 740px;
    }

    .pane-header,
    .terminal-header {
        height: 38px;
        display: grid;
        align-items: center;
        padding: 0 12px;
        border-bottom: 1px solid #1a2e4d;
        background: linear-gradient(180deg, #0f1a2f 0%, #0c1526 100%);
        color: #86a2c8;
        font-size: 12px;
    }

    .pane-header {
        grid-template-columns: 1fr auto;
    }

    .pane-header h2 {
        margin: 0;
        font-size: 12px;
        font-weight: 600;
        color: #88a9d5;
    }

    .pane-header button {
        border: 1px solid #315788;
        background: #0f1f39;
        color: #b5d3ff;
        border-radius: 6px;
        padding: 4px 10px;
        font-family: inherit;
        font-size: 12px;
        cursor: pointer;
    }

    .pane-header button:disabled {
        opacity: 0.6;
        cursor: wait;
    }

    textarea {
        width: 100%;
        min-height: calc(740px - 38px);
        border: 0;
        margin: 0;
        resize: vertical;
        background: #040912;
        color: #dce9ff;
        padding: 14px;
        box-sizing: border-box;
        font-family: inherit;
        font-size: 13px;
        line-height: 1.45;
        outline: none;
    }

    .terminal-header {
        grid-template-columns: 90px 1fr auto;
    }

    .lights {
        display: flex;
        gap: 7px;
    }

    .light {
        width: 10px;
        height: 10px;
        border-radius: 999px;
        display: inline-block;
    }

    .light.red {
        background: #ff6b8a;
    }

    .light.yellow {
        background: #f8cf68;
    }

    .light.green {
        background: #44d18d;
    }

    .title {
        text-align: center;
        color: #6ea0de;
    }

    .meta {
        color: #6f88ab;
    }

    .terminal-body {
        padding: 18px;
        background:
            radial-gradient(
                600px 240px at 20% 0%,
                rgba(70, 123, 214, 0.14) 0%,
                transparent 80%
            ),
            radial-gradient(
                700px 320px at 90% 70%,
                rgba(52, 92, 170, 0.12) 0%,
                transparent 80%
            ),
            #050a12;
        min-height: calc(740px - 38px);
        box-sizing: border-box;
    }

    .terminal-grid {
        position: relative;
        line-height: 1;
        font-size: 13px;
        width: calc(var(--cols) * 1ch);
        height: calc(var(--rows) * 1em);
        text-shadow: 0 0 10px rgba(110, 180, 255, 0.05);
        font-variant-ligatures: none;
        -webkit-font-smoothing: none;
        text-rendering: geometricPrecision;
    }

    .row {
        height: 1em;
        white-space: pre;
    }

    .cursor {
        position: absolute;
        width: 1ch;
        height: 1em;
        left: calc(var(--cursor-col) * 1ch);
        top: calc(var(--cursor-row) * 1em);
        background: rgba(240, 247, 255, 0.76);
        mix-blend-mode: screen;
        animation: blink 1.05s steps(1) infinite;
    }

    .error {
        margin: 0;
        color: #ffc3d0;
        white-space: pre-wrap;
        font-size: 13px;
        line-height: 1.4;
    }

    @keyframes blink {
        0%,
        49% {
            opacity: 0.9;
        }
        50%,
        100% {
            opacity: 0.15;
        }
    }

    @media (max-width: 1100px) {
        .workspace {
            grid-template-columns: 1fr;
        }

        .editor-pane,
        .terminal-shell {
            min-height: 560px;
        }

        textarea,
        .terminal-body {
            min-height: calc(560px - 38px);
        }

        .terminal-grid {
            font-size: 11px;
        }
    }
</style>
