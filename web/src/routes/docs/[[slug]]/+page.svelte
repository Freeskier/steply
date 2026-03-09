<script lang="ts">
    import { page } from "$app/state";

    import type { PageData } from "./$types";

    let { data }: { data: PageData } = $props();

    function typeTone(typeName: string): string {
        switch (typeName) {
            case "string":
                return "tone-string";
            case "boolean":
                return "tone-boolean";
            case "integer":
            case "number":
                return "tone-number";
            case "object":
                return "tone-object";
            case "array":
                return "tone-array";
            default:
                return "tone-default";
        }
    }
</script>

<svelte:head>
    <title>{data.doc.title} · Steply Docs</title>
    <meta
        name="description"
        content={`${data.doc.title} documentation for Steply.`}
    />
</svelte:head>

<div class="docs-shell">
    <div class="docs-noise"></div>

    <header class="docs-topbar">
        <div class="topbar-left">
            <a class="brand" href="/">
                <span class="brand-mark">s</span>
                <span class="brand-name">Steply</span>
            </a>
            <span class="topbar-divider">·</span>
            <a class="docs-link" href="/docs">Docs</a>
        </div>

        <div class="docs-breadcrumb">{data.doc.breadcrumb}</div>

        <div class="topbar-actions">
            <a class="ghost-link" href="/">← Back to home</a>
            <a class="primary-action" href="/creator">Open Creator ↗</a>
        </div>
    </header>

    <div class="docs-layout">
        <aside class="sidebar">
            {#each data.navSections as section}
                <section class="nav-section">
                    <div class="nav-heading">{section.title}</div>
                    <nav>
                        {#each section.items as item}
                            <a
                                href={item.href}
                                class:active-nav={page.url.pathname ===
                                    item.href}
                            >
                                <span class="nav-hash">#</span>
                                <span>{item.title}</span>
                            </a>
                        {/each}
                    </nav>
                </section>
            {/each}
        </aside>

        <main class="docs-main">
            <article class="docs-article">
                <p class="eyebrow">{data.doc.eyebrow}</p>
                <h1>{data.doc.title}</h1>
                <div class="article-rule"></div>

                {#if data.doc.kind === "static"}
                    <p class="lead">{data.doc.lead}</p>

                    {#each data.doc.sections as section}
                        <section class="content-section" id={section.id}>
                            <h2>{section.title}</h2>
                            {#each section.paragraphs as paragraph}
                                <p>{paragraph}</p>
                            {/each}

                            {#if section.code}
                                <div class="code-panel">
                                    <div class="code-head">
                                        <div class="code-meta">
                                            <span>{section.code.language}</span>
                                            <span>{section.code.label}</span>
                                        </div>
                                        <button type="button">copy</button>
                                    </div>
                                    <pre>{section.code.value}</pre>
                                </div>
                            {/if}
                        </section>
                    {/each}
                {:else}
                    <p class="lead">{data.doc.description}</p>

                    <section class="content-section" id="properties">
                        <h2>Properties</h2>
                        <table class="properties-table">
                            <thead>
                                <tr>
                                    <th>field</th>
                                    <th>type</th>
                                    <th>required</th>
                                    <th>description</th>
                                </tr>
                            </thead>
                            <tbody>
                                {#each data.doc.fields as field}
                                    <tr>
                                        <td class="field-name">{field.name}</td>
                                        <td>
                                            <span
                                                class={`type-pill ${typeTone(field.type_name)}`}
                                            >
                                                {field.type_name}
                                            </span>
                                        </td>
                                        <td class:required={field.required}>
                                            {field.required ? "yes" : "no"}
                                        </td>
                                        <td>
                                            <div class="field-desc">
                                                <span
                                                    >{field.short_description}</span
                                                >
                                                {#if field.default}
                                                    <small
                                                        >Default: {field.default}</small
                                                    >
                                                {/if}
                                                {#if field.allowed_values.length > 0}
                                                    <small>
                                                        Allowed: {field.allowed_values.join(
                                                            ", ",
                                                        )}
                                                    </small>
                                                {/if}
                                            </div>
                                        </td>
                                    </tr>
                                {/each}
                            </tbody>
                        </table>
                    </section>

                    {#if data.doc.hints.length > 0}
                        <section class="content-section" id="hints">
                            <h2>Hints</h2>
                            <div class="hint-list">
                                {#each data.doc.hints as hint}
                                    <div class="hint-card">
                                        <span>{hint.key}</span>
                                        <strong>{hint.label}</strong>
                                    </div>
                                {/each}
                            </div>
                        </section>
                    {/if}

                    <section class="content-section" id="example">
                        <h2>Example</h2>
                        <div class="code-panel">
                            <div class="code-head">
                                <div class="code-meta">
                                    <span>yaml</span>
                                    <span>example</span>
                                </div>
                                <button type="button">copy</button>
                            </div>
                            <pre>{data.doc.exampleYaml}</pre>
                        </div>
                    </section>
                {/if}
            </article>

            <footer class="article-footer">
                <span>steply docs · v0.9.2</span>
                <a href="/">← Back to landing</a>
            </footer>
        </main>

        <aside class="toc">
            <div class="toc-box">
                <p>On this page</p>
                <nav>
                    {#each data.doc.toc as item}
                        <a href={`#${item.id}`}>{item.title}</a>
                    {/each}
                </nav>
            </div>
        </aside>
    </div>
</div>

<style>
    :global(body) {
        background:
            radial-gradient(
                circle at 18% 18%,
                rgba(255, 190, 34, 0.06),
                transparent 18%
            ),
            linear-gradient(180deg, #151920 0%, #10141a 100%);
        color: #f2efe8;
    }

    :global(*) {
        box-sizing: border-box;
    }

    .docs-shell {
        position: relative;
        min-height: 100vh;
        background:
            linear-gradient(rgba(255, 255, 255, 0.028) 1px, transparent 1px),
            linear-gradient(
                90deg,
                rgba(255, 255, 255, 0.022) 1px,
                transparent 1px
            ),
            linear-gradient(
                180deg,
                rgba(16, 20, 26, 0.16),
                rgba(16, 20, 26, 0.12)
            );
        background-size:
            52px 52px,
            52px 52px,
            auto;
    }

    .docs-shell::before {
        content: "";
        position: absolute;
        inset: 0;
        pointer-events: none;
        background:
            radial-gradient(
                circle at 14% 22%,
                rgba(16, 20, 26, 0.34),
                transparent 24%
            ),
            radial-gradient(
                circle at 82% 56%,
                rgba(16, 20, 26, 0.24),
                transparent 24%
            ),
            linear-gradient(
                180deg,
                rgba(16, 20, 26, 0.18),
                rgba(16, 20, 26, 0.14)
            );
    }

    .docs-noise {
        position: absolute;
        inset: 0;
        pointer-events: none;
        background:
            repeating-linear-gradient(
                0deg,
                rgba(255, 255, 255, 0.01) 0,
                rgba(255, 255, 255, 0.01) 1px,
                transparent 2px,
                transparent 4px
            ),
            repeating-linear-gradient(
                90deg,
                transparent 0,
                transparent 11px,
                rgba(255, 255, 255, 0.006) 12px,
                transparent 13px
            );
        mix-blend-mode: soft-light;
        opacity: 0.28;
    }

    .docs-topbar {
        position: sticky;
        top: 0;
        z-index: 20;
        display: grid;
        grid-template-columns: 16rem 1fr auto;
        align-items: center;
        min-height: 3.8rem;
        padding: 0 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.07);
        background: rgba(15, 18, 24, 0.84);
        backdrop-filter: blur(14px);
    }

    .topbar-left,
    .topbar-actions {
        display: inline-flex;
        align-items: center;
        gap: 0.7rem;
    }

    .brand {
        display: inline-flex;
        align-items: center;
        gap: 0.8rem;
        color: #f2efe8;
        text-decoration: none;
        font-weight: 700;
    }

    .brand-mark {
        display: inline-grid;
        place-items: center;
        width: 1.8rem;
        height: 1.8rem;
        border-radius: 0.28rem;
        background: #f8be22;
        color: #151922;
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 1rem;
        text-transform: lowercase;
    }

    .brand-name,
    .docs-link {
        font-size: 1rem;
        color: #f2efe8;
        text-decoration: none;
    }

    .topbar-divider,
    .docs-breadcrumb {
        color: rgba(215, 210, 201, 0.44);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.94rem;
    }

    .docs-breadcrumb {
        justify-self: start;
    }

    .ghost-link,
    .toc a,
    .sidebar a,
    .article-footer a {
        color: rgba(225, 221, 213, 0.58);
        text-decoration: none;
    }

    .primary-action {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        min-height: 2.7rem;
        padding: 0 1.2rem;
        border: 1px solid rgba(255, 190, 34, 0.44);
        background: #f8be22;
        color: #171b23;
        font-weight: 700;
        text-decoration: none;
    }

    .docs-layout {
        display: grid;
        grid-template-columns: 16rem minmax(0, 1fr) 13rem;
        min-height: calc(100vh - 3.8rem);
        position: relative;
        z-index: 1;
    }

    .sidebar,
    .toc {
        position: relative;
        padding: 1.4rem 0;
    }

    .sidebar {
        border-right: 1px solid rgba(255, 255, 255, 0.07);
        padding-inline: 0.4rem;
    }

    .toc {
        border-left: 1px solid rgba(255, 255, 255, 0.07);
        padding-inline: 1rem;
    }

    .nav-section + .nav-section {
        margin-top: 1.4rem;
    }

    .nav-heading,
    .eyebrow,
    .toc-box p,
    .code-meta,
    .properties-table th {
        color: rgba(248, 190, 34, 0.7);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.78rem;
        letter-spacing: 0.12em;
        text-transform: uppercase;
    }

    .nav-section nav {
        display: grid;
        gap: 0.15rem;
        margin-top: 0.7rem;
    }

    .sidebar a {
        display: grid;
        grid-template-columns: 0.9rem 1fr;
        gap: 0.5rem;
        align-items: center;
        min-height: 2rem;
        padding: 0 0.8rem;
        border-left: 2px solid transparent;
    }

    .nav-hash {
        color: rgba(255, 255, 255, 0.18);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
    }

    .active-nav {
        border-left-color: rgba(248, 190, 34, 0.9);
        background: rgba(248, 190, 34, 0.12);
        color: #f2efe8;
    }

    .active-nav .nav-hash {
        color: rgba(248, 190, 34, 0.72);
    }

    .docs-main {
        padding: 3rem min(5vw, 4rem) 2rem;
    }

    .docs-article {
        width: min(100%, 47rem);
        margin: 0 auto;
    }

    h1,
    h2,
    p,
    pre,
    table {
        margin: 0;
    }

    h1 {
        margin-top: 0.8rem;
        font-size: clamp(2.6rem, 4vw, 3.4rem);
        line-height: 0.98;
        letter-spacing: -0.06em;
    }

    h2 {
        font-size: 1.7rem;
        letter-spacing: -0.04em;
        margin-bottom: 1rem;
    }

    .lead,
    .content-section p {
        color: rgba(229, 225, 217, 0.66);
        line-height: 1.65;
        font-size: 1rem;
    }

    .lead {
        margin-top: 1.3rem;
    }

    .article-rule {
        height: 1px;
        margin-top: 1.25rem;
        background: rgba(255, 255, 255, 0.07);
    }

    .content-section {
        margin-top: 2rem;
        scroll-margin-top: 5rem;
    }

    .content-section p + p {
        margin-top: 1rem;
    }

    .code-panel,
    .properties-table {
        margin-top: 1.1rem;
        border: 1px solid rgba(255, 255, 255, 0.08);
        background: rgba(10, 14, 20, 0.84);
        width: 100%;
    }

    .code-head {
        display: flex;
        justify-content: space-between;
        align-items: center;
        min-height: 2.4rem;
        padding: 0 0.9rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }

    .code-meta {
        display: inline-flex;
        gap: 0.7rem;
    }

    .code-head button {
        border: 0;
        background: transparent;
        color: rgba(225, 221, 213, 0.4);
        text-transform: lowercase;
    }

    .code-panel pre {
        padding: 1rem;
        color: rgba(227, 223, 214, 0.74);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        line-height: 1.6;
        white-space: pre-wrap;
    }

    .properties-table {
        border-collapse: collapse;
    }

    .properties-table th,
    .properties-table td {
        padding: 0.9rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        text-align: left;
        vertical-align: top;
    }

    .properties-table th {
        color: rgba(215, 210, 201, 0.42);
    }

    .field-name {
        color: rgba(248, 190, 34, 0.82);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
    }

    .type-pill {
        display: inline-flex;
        align-items: center;
        min-height: 1.5rem;
        padding: 0 0.5rem;
        border-radius: 0.25rem;
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.78rem;
        text-transform: lowercase;
    }

    .tone-string,
    .tone-default {
        background: rgba(47, 121, 191, 0.16);
        color: #7ec3ff;
    }

    .tone-boolean {
        background: rgba(248, 190, 34, 0.14);
        color: #f8be22;
    }

    .tone-number {
        background: rgba(120, 211, 170, 0.14);
        color: #78d3aa;
    }

    .tone-object {
        background: rgba(185, 133, 255, 0.14);
        color: #c49cff;
    }

    .tone-array {
        background: rgba(255, 142, 95, 0.14);
        color: #ffae83;
    }

    .required {
        color: rgba(248, 190, 34, 0.78);
    }

    .field-desc {
        display: grid;
        gap: 0.35rem;
        color: rgba(229, 225, 217, 0.62);
        line-height: 1.5;
    }

    .field-desc small {
        color: rgba(215, 210, 201, 0.42);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.78rem;
    }

    .hint-list {
        display: grid;
        gap: 0.75rem;
        margin-top: 1rem;
    }

    .hint-card {
        display: grid;
        gap: 0.35rem;
        padding: 0.9rem 1rem;
        border: 1px solid rgba(255, 255, 255, 0.08);
        background: rgba(18, 22, 29, 0.74);
    }

    .hint-card span {
        color: rgba(248, 190, 34, 0.78);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
    }

    .toc-box {
        position: sticky;
        top: 5.5rem;
    }

    .toc-box nav {
        display: grid;
        gap: 0.7rem;
        margin-top: 0.9rem;
        padding-left: 0.7rem;
        border-left: 1px solid rgba(255, 255, 255, 0.08);
    }

    .article-footer {
        display: flex;
        justify-content: space-between;
        gap: 1rem;
        margin-top: 4rem;
        padding-top: 1.5rem;
        border-top: 1px solid rgba(255, 255, 255, 0.07);
        color: rgba(215, 210, 201, 0.34);
        font-family: "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
        font-size: 0.9rem;
    }

    @media (max-width: 1200px) {
        .docs-layout {
            grid-template-columns: 15rem minmax(0, 1fr);
        }

        .toc {
            display: none;
        }
    }

    @media (max-width: 900px) {
        .docs-topbar {
            grid-template-columns: 1fr auto;
            gap: 1rem;
        }

        .docs-breadcrumb {
            display: none;
        }

        .docs-layout {
            grid-template-columns: 1fr;
        }

        .sidebar {
            border-right: 0;
            border-bottom: 1px solid rgba(255, 255, 255, 0.07);
            overflow-x: auto;
        }

        .nav-section nav {
            min-width: 14rem;
        }

        .docs-main {
            padding-inline: 1rem;
        }

        .topbar-actions .ghost-link {
            display: none;
        }
    }
</style>
