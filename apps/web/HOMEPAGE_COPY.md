# Steply Homepage Copy And Presentation Plan

This document is intentionally written from scratch and does not follow the current website.

Goal:
- make Steply feel like a serious terminal product, not a generic SaaS landing page
- show that it works in three modes: YAML flow, shell/script mode, and browser creator/preview
- make the homepage visually rich without turning it into random motion and generic gradients
- keep the page focused on product understanding, not marketing fluff

Important note:
- install commands and package manager names below use placeholders where release channels are not finalized yet
- replace them once the distribution strategy is locked

---

## Creative Direction

Steply should feel like:
- terminal-native
- programmable
- visual without being toy-like
- precise
- surprisingly broad: CLI prompt tool, YAML flow engine, live web preview, future creator

Visual language recommendation:
- sharp editorial layout, not soft SaaS cards everywhere
- restrained palette with one strong accent
- dark terminal surfaces mixed with warm paper-like content sections or muted graphite
- clear monospace presence, but balanced with a stronger display face for headlines

Recommended experience:
- the hero should immediately show code, a rendered terminal, and a "build flow visually" path
- the page should feel like the product is alive, not described from afar

---

## Homepage Structure

1. Hero
2. Install + Run Tabs
3. Why Steply / Core Value Proposition
4. Three Ways To Use It
5. Feature Showcase
6. How A Flow Comes Together
7. Creator Teaser
8. Examples Gallery
9. Docs / OSS / CTA Footer

---

## 1. Hero

### UI concept

Layout:
- left: headline, subheadline, CTAs
- right: an interactive showcase block
- inside showcase block: tabs that switch between `YAML`, `Shell`, `Creator`
- under tabs: code pane on the left and rendered terminal preview on the right

This is the most important component on the page.

It should communicate:
- you can write YAML
- you can script prompts from shell
- you can preview and later build flows visually in the browser

### Hero copy

Eyebrow:

`Terminal flows, prompts, and previews from one engine.`

Headline:

`Build terminal UX like a product, not a pile of prompts.`

Subheadline:

`Steply lets you create interactive terminal flows with YAML, compose them from shell scripts, and preview the exact runtime in the browser through the same rendering engine.`

Primary CTA:

`Install Steply`

Secondary CTA:

`Open Creator Preview`

Tertiary CTA:

`Read Docs`

### Hero install block

Use a tabbed install component directly in hero.

Tabs:
- `curl`
- `Homebrew`
- `npm`
- `cargo`

Recommended copy above install block:

`Install in one command. Then run a prompt, build a flow, or open the creator.`

Example code blocks:

```bash
curl -fsSL https://get.steply.dev/install.sh | sh
```

```bash
brew install steply
```

```bash
npm install -g steply
```

```bash
cargo install steply-cli
```

Small note under block:

`Use the same runtime locally, in scripts, and in the browser preview.`

### Hero live showcase tabs

#### Tab 1: YAML

Code:

```yaml
steps:
  - id: user
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
        label: Project directory
```

Rendered terminal:
- step frame
- two text inputs in one step
- next step visible in sidebar or progress rail

Caption:

`Write a flow declaratively in YAML.`

#### Tab 2: Shell

Code:

```bash
flow_id="$(steply flow create --decorate)"

steply flow step "$flow_id" --title "Basic user data"
steply text-input --flow "$flow_id" --target user.name --label "Name"
steply text-input --flow "$flow_id" --target user.email --label "Email"

steply flow run "$flow_id"
```

Rendered terminal:
- same runtime style as YAML tab
- same chrome
- same multi-input step idea

Caption:

`Assemble the same flow from shell when imperative logic is easier than config.`

#### Tab 3: Creator

UI mock concept:
- flow outline left
- widget settings right
- live terminal preview pane attached

Caption:

`Drag, configure, preview, export YAML.`

Small badge:

`Creator coming online`

### Extra hero details

Add a compact row under CTAs:
- `YAML-first`
- `Shell-friendly`
- `WASM preview`
- `Composable widgets`

Not as badges for decoration only.
They should look like concise capability signals.

---

## 2. Why Steply

### UI concept

Three sharp statement cards, horizontally on desktop, stacked on mobile.
Each card should feel like a thesis, not a feature bullet.

### Copy

Card 1:

Title:
`One engine, multiple entry points`

Body:
`Write a YAML flow, compose a draft flow from shell, or preview the same UI in the browser. The rendering model stays aligned.`

Card 2:

Title:
`Terminal UX with structure`

Body:
`Steply is not just a prompt picker. It gives you steps, validation, task hooks, embedded widgets, state, and a real interaction model.`

Card 3:

Title:
`Browser tooling without a fake preview`

Body:
`The web preview is not a hand-made approximation. It is powered by the same Rust/WASM rendering path that shapes the terminal runtime.`

---

## 3. Three Ways To Use It

### UI concept

A wide segmented component with three large modes.
Each mode has:
- title
- one-liner
- short code example
- one "best for" sentence

### Copy

#### YAML Flows

Title:
`Define full flows in YAML`

Body:
`Best when your interaction is mostly declarative and you want something easy to review, version, and document.`

Best for:
`Installers, setup wizards, onboarding flows, operator tooling.`

#### Shell Flows

Title:
`Compose flows from shell scripts`

Body:
`Best when control flow belongs in bash: conditionals, environment checks, branching, existing scripts, CI helpers.`

Best for:
`Ops scripts, dev bootstrap, interactive automation, release helpers.`

#### Creator

Title:
`Build visually, export clean YAML`

Body:
`Best when you want a lower-friction authoring path for people who should not hand-edit config but still need precise output.`

Best for:
`Docs demos, internal tooling, onboarding, fast iteration.`

---

## 4. Feature Showcase

### UI concept

Do not make this a boring icon grid.

Use a vertical feature rail with a sticky preview panel.
When the user scrolls the left column, the right side changes:
- YAML snippet
- terminal render
- a zoomed widget detail

### Recommended features

#### Structured Steps

Copy:

`Move beyond one-shot prompts. Group widgets into steps, carry state forward, validate transitions, and keep the flow readable.`

Visual:
- step timeline
- active step chrome
- multi-input step

#### Scriptable Flow Drafts

Copy:

`Use the same widgets from shell. Append them to a draft flow, group them into steps, then run the final interactive flow when you're ready.`

Visual:
- shell snippet on left
- generated flow structure on right

#### Shared Preview Engine

Copy:

`Preview the UI in the browser through WASM, without maintaining a fake frontend-only renderer.`

Visual:
- split view: YAML / terminal preview

#### Embedded Widgets

Copy:

`Handle more than plain text. Tables, repeaters, object editors, tree views, calendars, file browsers, snippets, and more.`

Visual:
- compact widget gallery wall

#### Task-Aware Runtime

Copy:

`Run commands, react to events, render progress, and keep flows connected to real work instead of static forms.`

Visual:
- command runner
- progress output
- task log

#### Docs And Creator Pipeline

Copy:

`Schema, docs JSON, browser preview, and future creator all build from the same model layer instead of drifting apart.`

Visual:
- simple pipeline diagram

---

## 5. How A Flow Comes Together

### UI concept

This section should be a real timeline.
Not decorative circles.

It should feel like a product pipeline:
- author
- preview
- run
- automate

### Copy

Step 1:

Title:
`Define`

Body:
`Start from YAML or compose a draft flow from shell commands.`

Step 2:

Title:
`Preview`

Body:
`Render the exact interaction model in the browser and iterate before shipping.`

Step 3:

Title:
`Run`

Body:
`Launch the flow in the terminal with real focus management, validation, state, and task hooks.`

Step 4:

Title:
`Scale`

Body:
`Generate docs, expose examples, and move toward a visual creator without rewriting the product model.`

---

## 6. Creator Teaser

### UI concept

This should feel aspirational, not vague.

Use a large section with:
- a mock creator screen
- cards for "drag widgets", "configure fields", "live terminal preview", "export YAML"
- a strong CTA to `/creator`

### Copy

Eyebrow:

`Creator`

Headline:

`Visual flow building without giving up clean output.`

Body:

`The creator is the visual layer on top of Steply's config model. Assemble steps, configure widgets, preview the terminal instantly, then export real YAML instead of locking your flow into a proprietary editor format.`

CTA:

`Open Creator`

Support text:

`Best for rapid iteration, internal tooling, and sharing flow design with non-Rust users.`

### UI notes

This is where you should show:
- one large creator mockup
- one mini terminal preview
- one YAML export panel

Do not hide it behind tiny screenshots.

---

## 7. Example Gallery

### UI concept

A carousel or 3-column gallery of concrete use cases.
Each example card should open into:
- short description
- YAML snippet
- terminal preview thumbnail

### Recommended examples

Example 1:
`Project bootstrap wizard`

Example 2:
`Deployment confirmation flow`

Example 3:
`Interactive ops script`

Example 4:
`Config editor with object editor + table`

Example 5:
`Repository scanner with file browser + command runner`

### Copy intro

`Steply is flexible enough for tiny prompts and structured enough for serious terminal flows.`

---

## 8. Install / Docs / OSS Footer Section

### UI concept

A strong bottom CTA area with three next actions side by side.

### Copy

Column 1:

Title:
`Install`

Body:
`Get the CLI and start with a single prompt or a full flow.`

Action:
`Copy install command`

Column 2:

Title:
`Read the docs`

Body:
`See widget fields, examples, schema details, and integration patterns.`

Action:
`Open Docs`

Column 3:

Title:
`Build visually`

Body:
`Use the creator and preview pipeline to design flows before shipping them to terminal users.`

Action:
`Open Creator`

Optional OSS line:

`Open source, Rust-powered, terminal-native.`

---

## Components Worth Building

These are the components that would make the homepage feel intentional instead of generic.

### 1. Install Command Switcher

Tabs:
- curl
- brew
- npm
- cargo

Behavior:
- one-click copy
- subtle success feedback
- remember last selected tab

### 2. Mode Switcher Showcase

Tabs:
- YAML
- Shell
- Creator

Behavior:
- switches both code panel and preview panel
- terminal render updates as the tab changes

### 3. Sticky Feature Rail

Behavior:
- left column scrolls features
- right preview stays pinned
- preview updates per feature

### 4. Flow Timeline

Behavior:
- visual pipeline, not a plain list
- each stage has a real artifact: YAML, preview, terminal, docs

### 5. Widget Mosaic

A visually dense section showing:
- text input
- file browser
- table
- repeater
- object editor
- tree view
- command runner
- task log

This should communicate breadth fast.

### 6. Creator Teaser Panel

Should show three synchronized panes:
- flow outline
- inspector
- terminal preview

### 7. Example Drawer

Each example card expands into:
- problem
- snippet
- preview
- CTA to docs or creator

---

## Motion Ideas

Use motion carefully.

Good motion:
- step progress line drawing in
- terminal cursor blink
- tab transitions between YAML / Shell / Creator
- staggered reveal of timeline stages
- subtle line sweep on copy button success

Bad motion:
- random floating blobs
- over-animated counters
- generic card hover explosions
- excessive parallax

---

## What To Avoid

- generic "developer tool" hero with a fake code background and no real product demonstration
- too many tiny cards with repeated copy
- empty claims like "blazing fast", "powerful", "modern"
- overexplaining implementation details before users understand the product
- making the creator the whole story too early

The homepage should first answer:
- what is Steply
- why it is different
- how I can use it today
- why it will scale with me later

---

## Suggested Final Flow Of The Page

1. Hero with install tabs and live YAML/Shell/Creator switcher
2. Three core value cards
3. Three ways to use Steply
4. Sticky feature rail with synced preview
5. How a flow comes together timeline
6. Creator teaser
7. Real-world examples gallery
8. Install / Docs / Creator closing CTA

---

## Tone Of Voice

The copy should feel:
- sharp
- technical
- confident
- not corporate
- not cute

Steply should sound like a serious tool for people who care about terminal UX, scripting, and architecture.

Good tone:
- `Build terminal UX like a product.`
- `Use the same engine in YAML, shell, and the browser.`
- `Preview first. Ship the real flow later.`

Bad tone:
- `Supercharge your workflows with next-generation prompt magic.`
- `Blazing-fast innovative solution for all your terminal needs.`

---

## Recommended Next Step

Before implementing UI, lock:
- install channels and exact commands
- whether homepage language is English-only
- whether Creator is framed as "preview", "beta", or "coming soon"
- 3 to 5 canonical examples to showcase

Once those are fixed, this markdown can be turned into:
- a homepage wireframe
- section-level component specs
- final production copy
