# Flow YAML Concept (v1)

## Cel
Zdefiniować cały flow i stepy w czytelnym YAML, który mapuje się na AST i runtime bez dodatkowych wyjątków.

## Założenia v1
1. Wszystko ma jawne `id` (stepy, widgety, taski).
2. Brak globalnego `vars` (warunki i taski bazują na wartościach widgetów/store).
3. Eventy/taski przez `subscriptions`.
4. Prosty routing wyniku taska: `target: <widget_id>`.
5. Conditional rendering stepów przez `when`.

## Struktura dokumentu
```yaml
version: 1

steps: []
flow: []
tasks: []
subscriptions: []
```

## Steps
Każdy step jest jawnie zdefiniowany przez `id`.

```yaml
steps:
  - id: project_search
    title: "Project search"
    description: "Optional"
    widgets:
      - type: text_input
        id: query
        label: "Query"
      - type: select_list
        id: results
        label: "Results"
        mode: list
        max_visible: 10
```

## Flow
`flow` określa kolejność i opcjonalne warunki pokazania stepu.

```yaml
flow:
  - step: project_search

  - step: deploy_checks
    when:
      all:
        - ref: deploy_env
          eq: prod
        - ref: user_role
          ne: viewer
```

## Conditional Rendering (`when`)
Wspierane formy (v1):
1. `all`
2. `any`
3. `not`
4. predykaty na `ref`

Przykłady:
```yaml
when:
  ref: query
  not_empty: true
```

```yaml
when:
  any:
    - ref: user_role
      eq: admin
    - ref: deploy_env
      eq: prod
```

```yaml
when:
  not:
    ref: debug_mode
    eq: true
```

## Tasks
Taski są niezależnymi definicjami wykonywania.

```yaml
tasks:
  - id: search_projects
    kind: exec
    program: python3
    args:
      - -c
      - |
        import json,sys
        q=(sys.argv[1] if len(sys.argv)>1 else "").strip().lower()
        data=[{"value":"api","title":"API","description":"Core service"},
              {"value":"worker","title":"Worker","description":"Background jobs"},
              {"value":"web","title":"Web","description":"Frontend app"}]
        out=[x for x in data if q in x["value"] or q in x["title"].lower()]
        print(json.dumps(out[:20]))
      - "${query}"
    parse: json
    timeout_ms: 12000
```

## Użycie wartości z inputów w taskach
Interpolacja przez placeholder `${widget_id}`.

Przykład:
```yaml
args:
  - "${query}"
```

Dodatkowa rekomendacja v1:
1. jeśli `id` nie istnieje -> błąd walidacji konfiguracji,
2. jeśli wartość pusta -> przekazujemy pusty string.

## Subscriptions
`subscriptions` łączą trigger z taskiem i wskazują, gdzie zapisać wynik.

```yaml
subscriptions:
  - task: search_projects
    trigger:
      on_input:
        ref: query
        debounce_ms: 180
    target: results
```

### Dlaczego `target`, nie `assign`
`target` jest prostsze semantycznie w v1: "wynik taska idzie do tego widgetu".

Potencjalne rozszerzenie v2 (bez łamania kompatybilności):
```yaml
target:
  id: results
  mode: append
```

## Command Runner
`command_runner` może wskazywać taski po `id`.

```yaml
steps:
  - id: deploy_checks
    title: "Deploy checks"
    widgets:
      - type: command_runner
        id: predeploy_runner
        label: "Pre-deploy checks"
        run_mode: manual
        on_error: stay
        commands:
          - task: lint
            label: "Run lint"
          - task: tests
            label: "Run tests"
          - task: smoke
            label: "Run smoke"

tasks:
  - id: lint
    kind: exec
    program: bash
    args: ["-lc", "cargo clippy --all-targets --all-features -q"]

  - id: tests
    kind: exec
    program: bash
    args: ["-lc", "cargo test -q"]

  - id: smoke
    kind: exec
    program: bash
    args: ["-lc", "echo smoke-ok"]
```

## Pełny przykład (v1)
```yaml
version: 1

steps:
  - id: project_search
    title: "Project search"
    widgets:
      - type: text_input
        id: query
        label: "Query"
        placeholder: "Type project name..."
      - type: select_list
        id: results
        label: "Results"
        mode: list
        max_visible: 10

  - id: deploy_checks
    title: "Deploy checks"
    widgets:
      - type: select
        id: deploy_env
        label: "Environment"
        options: [dev, staging, prod]
      - type: text_input
        id: user_role
        label: "User role"
      - type: command_runner
        id: predeploy_runner
        label: "Pre-deploy checks"
        run_mode: manual
        on_error: stay
        commands:
          - task: lint
            label: "Run lint"
          - task: tests
            label: "Run tests"

tasks:
  - id: search_projects
    kind: exec
    program: python3
    args:
      - -c
      - |
        import json,sys
        q=(sys.argv[1] if len(sys.argv)>1 else "").strip().lower()
        data=[{"value":"api","title":"API","description":"Core service"},
              {"value":"worker","title":"Worker","description":"Background jobs"}]
        out=[x for x in data if q in x["value"] or q in x["title"].lower()]
        print(json.dumps(out[:20]))
      - "${query}"
    parse: json

  - id: lint
    kind: exec
    program: bash
    args: ["-lc", "cargo clippy --all-targets --all-features -q"]

  - id: tests
    kind: exec
    program: bash
    args: ["-lc", "cargo test -q"]

subscriptions:
  - task: search_projects
    trigger:
      on_input:
        ref: query
        debounce_ms: 180
    target: results

flow:
  - step: project_search

  - step: deploy_checks
    when:
      all:
        - ref: deploy_env
          eq: prod
        - ref: user_role
          ne: viewer
```

## Kierunek AST
Rekomendacja: parser YAML -> AST -> walidacja referencji -> kompilacja do runtime.

Minimalny AST powinien obejmować:
1. `steps` i `widgets`
2. `flow` + `when`
3. `tasks`
4. `subscriptions` (trigger + target)

To daje prosty i stabilny fundament pod kolejne rozszerzenia (append/merge/path target, więcej triggerów, condition operators).
