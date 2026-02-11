# Steply V2 Plan

## 1. Cel

V2 ma uprościć przepływ aplikacji i ujednolicić kontrakty między `App`, `Reducer`, `FormEngine`, `Component` i `UI`.

Najważniejsze cele:
- Jeden czytelny pipeline zdarzeń.
- Brak ukrytych side-effectów między komponentami.
- Wyraźny podział odpowiedzialności między warstwami.
- Łatwiejsze testowanie i dodawanie nowych komponentów.

## 2. Docelowy Flow Runtime

Docelowy przepływ:

1. `Terminal` mapuje wejście do `TerminalEvent`.
2. `App` zamienia event na `Command` (tylko mapping i routing warstwy aktywnej).
3. `Reducer` przetwarza `Command` na:
- mutację `AppState`
- listę `Effect`
4. `Runtime` wykonuje `Effect` (np. emit event, async job, clear error).
5. `RenderPipeline` renderuje `ViewModel` wygenerowany ze stanu.

Reguła: tylko `Reducer` mutuje stan domenowy.

## 3. Moduły V2

Proponowana struktura:

```text
v2/
  PLAN.md
src/
  app/
    runtime.rs        # loop, tick, dispatch, effect executor
    commands.rs       # Command + mapowanie skrótów
    events.rs         # AppEvent/DomainEvent
  state/
    app_state.rs      # global state
    step_state.rs     # stan kroku
    focus_state.rs    # focus targets i active id
  domain/
    reducer.rs        # jedyne miejsce mutacji stanu
    effects.rs        # Effect enum
    validation.rs
    bindings.rs
  ui/
    view_model.rs
    renderer.rs
    layout.rs
  components/
    mod.rs
    traits.rs
    builtins/
  inputs/
    mod.rs
    traits.rs
    builtins/
  terminal/
    terminal.rs
    input_event.rs
```

## 4. Główne Zasady Architektury

1. `App` nie zawiera logiki biznesowej formularza.
2. `Reducer` jest deterministyczny (brak I/O w reducerze).
3. `Component` nie mutuje innych węzłów bezpośrednio po ID.
4. Komunikacja komponentów tylko przez typed message (`ComponentEvent`).
5. `Binding` działa przez jawne kanały danych (`OutputPort -> InputPort`), nie przez search po drzewie przy każdym key press.
6. `Focus` ma jeden model dla inputów i komponentów grupowych.

## 5. Kontrakty Traitów (szkic)

### 5.1 Input

```rust
pub trait Input: Send {
    fn id(&self) -> &str;
    fn focus(&mut self, focused: bool);

    fn view(&self) -> InputView;
    fn value(&self) -> Value;

    fn on_key(&mut self, key: KeyEvent) -> InputResult;
    fn validate(&self) -> Result<(), ValidationError>;
}

pub enum InputResult {
    Ignored,
    Changed { value: Value },
    SubmitRequested,
}
```

### 5.2 Component

```rust
pub trait Component: Send {
    fn id(&self) -> &str;
    fn focus_mode(&self) -> FocusMode;
    fn focus(&mut self, focused: bool);

    fn children(&self) -> Option<&[Node]>;
    fn children_mut(&mut self) -> Option<&mut [Node]>;

    fn view(&self, ctx: &ViewContext) -> ComponentView;
    fn on_key(&mut self, key: KeyEvent) -> ComponentResult;
    fn on_tick(&mut self) -> ComponentResult { ComponentResult::none() }
}

pub enum FocusMode {
    Container, // fokus idzie do children
    Group,     // komponent sam jest fokusowalny
}

pub struct ComponentResult {
    pub handled: bool,
    pub events: Vec<ComponentEvent>,
}

pub enum ComponentEvent {
    ValueProduced { port: String, value: Value },
    RequestSubmit,
    RequestFocus { target: FocusTarget },
}
```

### 5.3 Layer

```rust
pub trait Layer: Send {
    fn id(&self) -> &str;
    fn mode(&self) -> LayerMode;

    fn nodes(&self) -> &[Node];
    fn nodes_mut(&mut self) -> &mut [Node];

    fn on_open(&mut self, _ctx: &mut LayerCtx) {}
    fn on_close(&mut self, _ctx: &mut LayerCtx) {}
    fn on_key(&mut self, key: KeyEvent, ctx: &mut LayerCtx) -> bool;
}

pub enum LayerMode {
    Modal,
    Shared,
}
```

### 5.4 Reducer + Effects

```rust
pub trait Reducer {
    fn reduce(state: &mut AppState, cmd: Command) -> Vec<Effect>;
}

pub enum Effect {
    Emit(AppEvent),
    EmitAfter { event: AppEvent, delay_ms: u64 },
    StartJob(JobRequest),
    CancelJob(JobId),
    RequestRender,
}
```

## 6. Model Danych

`AppState` powinien trzymać:
- `flow: FlowState`
- `active_layer: Option<LayerState>`
- `focus: FocusState`
- `errors: ErrorState`
- `bindings: BindingGraph`
- `jobs: JobRegistry`

Zasada:
- dane formularza i nawigacji w `AppState`
- lokalny stan komponentu tylko jeśli dotyczy jego wewnętrznej prezentacji/interakcji

## 7. Focus i Binding

### Focus
- Prekomputowana lista `FocusTarget` dla aktywnego drzewa.
- `Tab` i `Shift+Tab` działają tylko na tej liście.
- Wejście do/wyjście z layera przebudowuje focus graph.

### Binding
- Każdy komponent może wystawiać `OutputPort`.
- Inputy/komponenty deklarują `InputPort`.
- Runtime przekazuje `Value` przez `BindingGraph`.
- Brak bezpośredniego `find_input_mut(id)` w logice komponentu.

## 8. Renderowanie

1. `ViewModelBuilder` tworzy płaski model renderu z `AppState`.
2. `Renderer` mapuje `ViewModel` -> `Span/Line`.
3. `TerminalRenderer` robi wyłącznie I/O terminala.

Zasada: brak logiki domenowej w rendererze.

## 9. Testy

Minimalny zestaw testów v2:
- Reducer unit tests: submit, focus move, validation failure.
- Focus tests: tab order, group/container behavior.
- Binding tests: event z komponentu aktualizuje target port.
- Integration tests: key sequence -> expected state snapshot.

## 10. Plan Migracji (bez big-bang)

Etap 1:
- Nowy runtime + reducer + state + 2 inputy (`TextInput`, `CheckboxInput`).

Etap 2:
- Walidacja, binding graph, warstwy (`Layer`).

Etap 3:
- Komponenty złożone (`Select`, `Table`, `Tree`, `FileBrowser`).

Etap 4:
- Zastąpienie demo flow i usunięcie starych adapterów.

## 11. Decyzje Projektowe Do Potwierdzenia

1. Czy `Group` component ma mieć własny cursor/render field, czy zawsze delegować do child view?
2. Czy `Value` zostaje jako enum globalny, czy robimy typed per-port z konwersjami?
3. Czy async jobs (np. file scanning) trzymamy w jednym executorze runtime, czy per-component worker?
4. Czy binding ma wspierać transformacje (map/filter), czy tylko 1:1 pass-through w MVP?
