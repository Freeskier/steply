import docsJson from "$lib/generated/config.docs.json";

type FieldDoc = {
    name: string;
    type_name: string;
    required: boolean;
    short_description: string;
    long_description: string | null;
    default: string | null;
    allowed_values: string[];
};

type StaticHint = {
    key: string;
    label: string;
};

type WidgetDoc = {
    widget_type: string;
    category: string;
    short_description: string;
    long_description: string;
    example_yaml: string;
    static_hints: StaticHint[];
    fields: FieldDoc[];
};

type DocsJson = {
    version: number;
    widgets: WidgetDoc[];
    embedded_widgets: WidgetDoc[];
};

export type TocItem = {
    id: string;
    title: string;
};

export type NavItem = {
    title: string;
    href: string;
};

export type NavSection = {
    title: string;
    items: NavItem[];
};

export type StaticSection = {
    id: string;
    title: string;
    paragraphs: string[];
    code?: {
        language: string;
        label: string;
        value: string;
    };
};

export type StaticDocPage = {
    kind: "static";
    slug: string;
    breadcrumb: string;
    eyebrow: string;
    title: string;
    lead: string;
    sections: StaticSection[];
    toc: TocItem[];
};

export type WidgetDocPage = {
    kind: "widget";
    slug: string;
    breadcrumb: string;
    eyebrow: string;
    title: string;
    lead: string;
    description: string;
    fields: FieldDoc[];
    exampleYaml: string;
    hints: StaticHint[];
    toc: TocItem[];
};

export type DocPage = StaticDocPage | WidgetDocPage;

const docs = docsJson as DocsJson;

function titleCase(value: string): string {
    return value
        .split(/[_-\s]+/)
        .filter(Boolean)
        .map((part) => part[0].toUpperCase() + part.slice(1))
        .join(" ");
}

function widgetSlug(widgetType: string): string {
    return `widget-${widgetType.replaceAll("_", "-")}`;
}

function widgetLabel(widgetType: string): string {
    return titleCase(widgetType.replace(/_/g, " "));
}

const staticPages: Record<string, StaticDocPage> = {
    overview: {
        kind: "static",
        slug: "overview",
        breadcrumb: "steply / overview",
        eyebrow: "Getting started",
        title: "Overview",
        lead: "Steply is a schema-first flow builder for YAML-defined terminal interfaces. You describe your steps, widgets, and logic in a single contract while the runtime handles rendering, validation, navigation, and output collection.",
        sections: [
            {
                id: "quickstart",
                title: "Quickstart",
                paragraphs: [
                    "You can use Steply in three ways: define a declarative flow in YAML, assemble the same flow from shell scripts, or configure it visually in the creator and export clean YAML.",
                    "The important architectural constraint is that all three paths converge into the same runtime model. Docs, generated schema, CLI flags, and preview behavior all hang off the same config layer rather than being maintained separately.",
                ],
            },
            {
                id: "why-steply",
                title: "Why Steply",
                paragraphs: [
                    "Steply is not a thin prompt wrapper. It gives you real steps, focus movement, validation on transitions, task hooks, shared state, embedded widgets, and a deterministic interaction model.",
                    "That matters because terminal tooling tends to start as one prompt and quickly turns into a product surface. Steply is designed for that point in the curve, not just the first prompt.",
                ],
            },
            {
                id: "example",
                title: "Example",
                paragraphs: [
                    "A minimal multi-step flow still reads like a contract. The same structure is what powers the browser preview and creator export path.",
                ],
                code: {
                    language: "yaml",
                    label: "hello-world.yaml",
                    value: `steps:
  - id: name
    title: Basic user data
    widgets:
      - type: text_input
        id: user_name
        label: What's your name?
        submit_target: user.name
      - type: select_input
        id: user_lang
        label: Preferred language

  - id: confirm
    title: Confirm
    widgets:
      - type: confirm_input
        id: generate
        label: Generate project for {{user.name}}?`,
                },
            },
        ],
        toc: [
            { id: "quickstart", title: "Quickstart" },
            { id: "why-steply", title: "Why Steply" },
            { id: "example", title: "Example" },
        ],
    },
    quickstart: {
        kind: "static",
        slug: "quickstart",
        breadcrumb: "steply / quickstart",
        eyebrow: "Getting started",
        title: "Quickstart",
        lead: "Install the CLI, run a single prompt, then move to a real flow when your interaction grows beyond one question.",
        sections: [
            {
                id: "install",
                title: "Install",
                paragraphs: [
                    "Use the install channel that fits your environment. The CLI binary is the same product surface regardless of whether it came from curl, Homebrew, npm, or Cargo.",
                ],
                code: {
                    language: "bash",
                    label: "install",
                    value: `curl -fsSL https://get.steply.dev/install.sh | sh
steply text-input --label "Project name"`,
                },
            },
            {
                id: "first-flow",
                title: "First flow",
                paragraphs: [
                    "Once you need multiple inputs, step titles, or validation boundaries, move to a real flow definition instead of chaining prompts by hand.",
                ],
                code: {
                    language: "yaml",
                    label: "flow.yaml",
                    value: `steps:
  - id: basic
    title: Basic user data
    widgets:
      - type: text_input
        id: name
        label: Name
      - type: text_input
        id: email
        label: Email`,
                },
            },
        ],
        toc: [
            { id: "install", title: "Install" },
            { id: "first-flow", title: "First flow" },
        ],
    },
    "yaml-structure": {
        kind: "static",
        slug: "yaml-structure",
        breadcrumb: "steply / yaml-structure",
        eyebrow: "Getting started",
        title: "YAML Structure",
        lead: "A flow is a set of ordered steps. Each step contains widgets, and each widget maps to a typed schema definition.",
        sections: [
            {
                id: "steps",
                title: "Steps",
                paragraphs: [
                    "Steps are the primary UX boundary. They control how widgets are grouped, how validation gates transitions, and how the user perceives progress in the flow.",
                ],
            },
            {
                id: "widgets",
                title: "Widgets",
                paragraphs: [
                    "Each widget definition is schema-backed. That means fields, docs, static hints, and CLI flag surfaces can be derived from the same model.",
                ],
            },
            {
                id: "targets",
                title: "Targets and state",
                paragraphs: [
                    "Submit targets let you write collected values into explicit paths in the flow store instead of relying only on raw widget ids.",
                ],
            },
        ],
        toc: [
            { id: "steps", title: "Steps" },
            { id: "widgets", title: "Widgets" },
            { id: "targets", title: "Targets and state" },
        ],
    },
    conditions: {
        kind: "static",
        slug: "conditions",
        breadcrumb: "steply / conditions",
        eyebrow: "Runtime API",
        title: "Conditions (when)",
        lead: "Condition expressions decide whether parts of a flow should be visible or traversable based on the current value store.",
        sections: [
            {
                id: "overview",
                title: "Overview",
                paragraphs: [
                    "Use `when` to gate steps or widgets on the current store state. This keeps branching logic in the flow contract instead of scattering it into unrelated code paths.",
                ],
            },
        ],
        toc: [{ id: "overview", title: "Overview" }],
    },
    tasks: {
        kind: "static",
        slug: "tasks",
        breadcrumb: "steply / tasks",
        eyebrow: "Runtime API",
        title: "Tasks",
        lead: "Tasks connect the flow to real work: command execution, progress rendering, state updates, and runtime reactions.",
        sections: [
            {
                id: "overview",
                title: "Overview",
                paragraphs: [
                    "A task-aware runtime lets the flow do more than collect values. It can run commands, stream progress, and bind transitions to actual system work.",
                ],
            },
        ],
        toc: [{ id: "overview", title: "Overview" }],
    },
    "target-binding": {
        kind: "static",
        slug: "target-binding",
        breadcrumb: "steply / target-binding",
        eyebrow: "Runtime API",
        title: "Target Binding",
        lead: "Targets let you store collected widget values into explicit paths rather than only synchronizing by widget id.",
        sections: [
            {
                id: "overview",
                title: "Overview",
                paragraphs: [
                    "This becomes especially useful in shell-built flows, where `--target` provides a stable path in the global store and keeps the final output shape intentional.",
                ],
            },
        ],
        toc: [{ id: "overview", title: "Overview" }],
    },
    hints: {
        kind: "static",
        slug: "hints",
        breadcrumb: "steply / hints",
        eyebrow: "Runtime API",
        title: "Hints",
        lead: "Static hints describe the stable input affordances of a widget and are shared between docs and the runtime surface.",
        sections: [
            {
                id: "overview",
                title: "Overview",
                paragraphs: [
                    "Steply separates static hint definitions from dynamic runtime context so docs stay trustworthy while widgets can still add contextual behavior when needed.",
                ],
            },
        ],
        toc: [{ id: "overview", title: "Overview" }],
    },
    "field-types": {
        kind: "static",
        slug: "field-types",
        breadcrumb: "steply / field-types",
        eyebrow: "Reference",
        title: "Field Types",
        lead: "Docs and CLI surfaces normalize widget fields into stable type names such as string, integer, boolean, object, and array.",
        sections: [
            {
                id: "overview",
                title: "Overview",
                paragraphs: [
                    "Those normalized types are what drive property tables, generated docs JSON, and generic CLI flag help for widget commands.",
                ],
            },
        ],
        toc: [{ id: "overview", title: "Overview" }],
    },
    expressions: {
        kind: "static",
        slug: "expressions",
        breadcrumb: "steply / expressions",
        eyebrow: "Reference",
        title: "Expressions",
        lead: "Expressions are used in conditions, validation, and other places where runtime state needs to influence the flow contract.",
        sections: [
            {
                id: "overview",
                title: "Overview",
                paragraphs: [
                    "Selectors and expressions should remain readable because they become part of the long-term behavior surface of the flow, not just a temporary implementation detail.",
                ],
            },
        ],
        toc: [{ id: "overview", title: "Overview" }],
    },
    "cli-reference": {
        kind: "static",
        slug: "cli-reference",
        breadcrumb: "steply / cli-reference",
        eyebrow: "Reference",
        title: "CLI Reference",
        lead: "The CLI supports single-widget prompt mode, full YAML flows, generated widget help, and shell-built flow drafts.",
        sections: [
            {
                id: "overview",
                title: "Overview",
                paragraphs: [
                    "Use `steply run --config ...` for declarative flows, or use widget commands directly when you want gum-like scripting with Steply's runtime surface underneath.",
                ],
                code: {
                    language: "bash",
                    label: "cli",
                    value: `steply text-input --label "Project name"
steply flow create --decorate
steply flow step "$id" --title "Basic user data"`,
                },
            },
        ],
        toc: [{ id: "overview", title: "Overview" }],
    },
};

const widgetPages: WidgetDocPage[] = docs.widgets
    .map((widget) => ({
        kind: "widget" as const,
        slug: widgetSlug(widget.widget_type),
        breadcrumb: `steply / ${widgetSlug(widget.widget_type)}`,
        eyebrow: "Widget",
        title: widgetLabel(widget.widget_type),
        lead: widget.short_description,
        description: widget.long_description || widget.short_description,
        fields: widget.fields,
        exampleYaml: widget.example_yaml,
        hints: widget.static_hints,
        toc: [
            { id: "properties", title: "Properties" },
            ...(widget.static_hints.length > 0
                ? [{ id: "hints", title: "Hints" }]
                : []),
            { id: "example", title: "Example" },
        ],
    }))
    .sort((a, b) => a.title.localeCompare(b.title));

const widgetPageMap = new Map(widgetPages.map((page) => [page.slug, page]));

export function getDocPage(slug?: string): DocPage | null {
    if (!slug) {
        return staticPages.overview;
    }

    return staticPages[slug] ?? widgetPageMap.get(slug) ?? null;
}

export function getNavSections(): NavSection[] {
    return [
        {
            title: "Getting started",
            items: [
                { title: "Overview", href: "/docs" },
                { title: "Quickstart", href: "/docs/quickstart" },
                { title: "YAML Structure", href: "/docs/yaml-structure" },
            ],
        },
        {
            title: "Widgets",
            items: widgetPages.map((page) => ({
                title: page.title,
                href: `/docs/${page.slug}`,
            })),
        },
        {
            title: "Runtime API",
            items: [
                { title: "Conditions (when)", href: "/docs/conditions" },
                { title: "Tasks", href: "/docs/tasks" },
                { title: "Target binding", href: "/docs/target-binding" },
                { title: "Hints", href: "/docs/hints" },
            ],
        },
        {
            title: "Reference",
            items: [
                { title: "Field types", href: "/docs/field-types" },
                { title: "Expressions", href: "/docs/expressions" },
                { title: "CLI reference", href: "/docs/cli-reference" },
            ],
        },
    ];
}
