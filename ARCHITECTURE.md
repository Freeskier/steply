# Steply Architecture

This document describes how the application is structured and how data, focus, events, and rendering flow through the system. It reflects the current tree‑based UI model with `FocusMode` and component grouping.

## 1. High‑Level Flow (Runtime Loop)

1. `src/main.rs` creates a `Terminal`, configures raw mode, and enters the event loop.
2. The loop reads terminal events (`TerminalEvent`) and forwards key presses to `App::handle_key`.
3. `App` queues events in `EventQueue` and processes them in `tick()`.
4. `App::render` draws the current step and optional overlay via `RenderPipeline`.
5. The loop exits when `App::should_exit` becomes true.

The app is event‑driven, but the **source of truth** is the in‑memory state in `AppState`, not the terminal.

## 2. Core Concepts

### 2.1 Node Tree (UI Structure)

The UI is a tree of `Node` objects (`src/core/node.rs`). A `Step` contains a **root list** of nodes (`Vec<Node>`), each of which can be:

- `Node::Input(Box<dyn Input>)`
- `Node::Component(Box<dyn Component>)`
- `Node::Text(String)`
- `Node::Separator`

Components may own children (via `children()` / `children_mut()`), which makes the tree recursive.

#### Why tree?
It allows **clean composition**: components can embed inputs and other components while keeping focus, render, and validation traversal consistent.

### 2.2 Widget (Shared Base for Input/Component)

`Widget` (`src/core/widget.rs`) is a small shared trait that unifies the overlap between `Input` and `Component`:

- `id()`
- `is_focused()` / `set_focused()`
- `key_caps()`

`Node` uses this to treat inputs and components consistently when it needs to read focus state or key capabilities.

### 2.2 Input and Component

- `Input` (`src/inputs/input.rs`) represents a focusable field with value, cursor, validators, and rendering content.
- `Component` (`src/core/component.rs`) represents a higher‑level UI unit. It can be **focusable** or **pass‑through** depending on `FocusMode`.

#### FocusMode
Defined in `src/core/component.rs`:

- `PassThrough` (default): focus traverses the component’s children.
- `Group`: the component itself is focusable and routes keys internally.

This enables **focus groups**, e.g. a filter input + list where letters go to the input and arrows go to the list.

#### EventContext
`EventContext` (`src/core/component.rs`) is the mechanism for components to emit updates in a uniform way:

- `ctx.update_input(id, value)` → produces a normal `InputChanged` flow for a child input
- `ctx.produce(value)` → emits a component value (bindable via `BindTarget`)
- `ctx.submit()` → request submit
- `ctx.handled()` → mark that the key was handled

This keeps component key handling clean and avoids manual `ComponentResponse` assembly.

### 2.3 Step and Flow

- `Step` (`src/core/step.rs`) is a screen with prompt, hint, nodes, and form validators.
- `Flow` (`src/core/flow.rs`) manages a list of steps, current index, and step status.

Steps are built with `StepBuilder` (`src/core/step_builder.rs`). It constructs a flat list of root nodes which can contain nested children.

### 2.4 Layers (Overlay)

- `Layer` (`src/core/layer.rs`) is a temporary UI layer (e.g. overlay search).
- `LayerManager` (`src/core/layer_manager.rs`) swaps the active node tree between the step and the overlay.

When a layer is active, **focus and input are scoped to the layer’s node tree**, while the step remains rendered underneath.

## 3. Input and Event System

### 3.1 Key Events

- `Terminal` (`src/terminal/terminal.rs`) maps crossterm events to internal `KeyEvent` (`src/terminal/input_event.rs`).
- `App::handle_key` enqueues `AppEvent::Key`.

### 3.2 Event Queue

- `EventQueue` (`src/core/event_queue.rs`) holds events in FIFO order and can schedule delayed events.
- `App::tick` drains ready events and dispatches them.

### 3.3 Action Bindings

- `ActionBindings` (`src/core/action_bindings.rs`) maps specific keys (Tab, Shift+Tab, Ctrl+Backspace, etc.) to actions.
- Captured keys are determined by **focused node key capabilities** (see below).

**Update:** `KeyCaps` was removed. The rule is now simple:

1. If a key matches a global action binding, it is handled as an action.
2. Otherwise it is forwarded to the focused widget/input.

`Ctrl+Backspace` and `Ctrl+Delete` are handled globally and call `input.delete_word()` / `delete_word_forward()` on the focused input. Inputs override those methods when they need custom behavior.

### 3.4 Key Capture (KeyCaps)

`KeyCaps` has been removed in favor of the simpler global‑first routing described above.

This allows text inputs to own word deletion (`Ctrl+Backspace`) while still letting global key bindings work elsewhere.

### 3.5 Reducer and Effects

- `Reducer` (`src/core/reducer.rs`) is the main state transition engine.
- It processes `Action` and returns `Effect` objects:
  - `Emit` events immediately
  - `EmitAfter` to schedule events
  - `CancelClearError` to cancel scheduled error clearing
  - `ComponentProduced` when a component emits a value

`App::apply_effects` executes these effects.

### 3.6 Form Engine (Focus + Input Changes)

`FormEngine` (`src/core/form_engine.rs`) owns all **focus traversal** and **input mutation** logic.

Key responsibilities:

- **Build focus targets** by traversing the node tree:
  - `Input` nodes are focusable
  - `Component` nodes are focusable only if `FocusMode::Group`
- Maintain `focus_index` and focused node path
- Handle `Tab` traversal
- Dispatch keys to the focused node

`FormEngine::handle_key` returns an `EngineOutput`:

- `events`: `FormEvent` like `InputChanged`, `FocusChanged`, `SubmitRequested`
- `produced`: values emitted by components (for bindings)

### 3.7 Component‑Driven Input Changes

`ComponentResponse` can include `changes`, which allow **group components** to modify child inputs while still producing normal `InputChanged` events. This is how a focus group can route keystrokes to a child input without the engine directly focusing that child.

## 4. Validation Flow

- `validation::validate_all_inputs(step)` traverses the tree and validates every input.
- Step‑level validators (`FormValidator`) are applied after input validation.
- Errors are applied by `FormEngine::apply_errors` and scheduled for clearing via `Effect::EmitAfter`.

## 5. Render Pipeline

### 5.1 Render Context

`RenderContext` (`src/ui/render/step_builder.rs`) provides access to theme and rendering helpers:

- `render_node_lines` handles recursion for components and children
- `render_input_full` and `render_input_field` draw inputs with cursor logic

Components render via `Component::render(&RenderContext)`.

### 5.2 StepRenderer

- Builds the final list of `RenderLine` objects for a step.
- Special case: inline input when the step has exactly one node and a prompt.

### 5.3 Layout

`Layout` (`src/ui/layout.rs`) wraps spans into a `Frame`, handling line wrapping and cursor positioning.

### 5.4 RenderPipeline

`RenderPipeline` (`src/ui/render/pipeline.rs`) is responsible for:

- Decoration (prompt/status glyphs)
- Writing to the terminal
- Cursor placement
- Rendering overlays on top of steps

The pipeline uses:

- `Decorator` (`src/ui/render/decorator.rs`) for status glyphs
- `RenderOptions` (`src/ui/render/options.rs`) for status‑specific styles

## 6. Data Flow Summary

**User key press → App → Reducer → FormEngine → Effects → Events → Render**

1. Key press becomes `AppEvent::Key`.
2. `App` checks for special cases (overlay toggle, tab completion, submit, action bindings).
3. Remaining keys become `Action::InputKey` and go through `Reducer`.
4. `FormEngine` mutates focused node, producing `FormEvent`.
5. `Reducer` converts `FormEvent` into `Effect` objects.
6. `App::apply_effects` emits events and updates state.
7. Rendering uses the updated state tree.

## 7. File Responsibilities (Quick Map)

### Core
- `src/core/app.rs`: main application coordinator (events, render, bindings, overlay).
- `src/core/state.rs`: ties `Flow` and `FormEngine` together.
- `src/core/flow.rs`: step sequencing + status.
- `src/core/step.rs`: step data (prompt, hint, nodes, validators).
- `src/core/step_builder.rs`: ergonomic step creation.
- `src/core/node.rs`: node tree + search utilities.
- `src/core/component.rs`: component base, focus mode, response types.
- `src/core/form_engine.rs`: focus traversal + input mutation.
- `src/core/reducer.rs`: central action reducer.
- `src/core/event.rs`: action enum.
- `src/core/event_queue.rs`: event scheduling and dispatch.
- `src/core/validation.rs`: input and form validation.
- `src/core/value.rs`: polymorphic values passed between inputs/components.
- `src/core/layer.rs`: overlay abstraction.
- `src/core/layer_manager.rs`: manages active overlay and focus transitions.

### Inputs
- `src/inputs/input.rs`: base trait and common input behavior.
- `src/inputs/*`: concrete inputs (text, array, segmented, etc.).
- `src/inputs/validators.rs`: reusable validators.

### UI
- `src/ui/render/step_builder.rs`: render context + step rendering.
- `src/ui/render/pipeline.rs`: terminal drawing pipeline.
- `src/ui/render/decorator.rs`: status glyph decorations.
- `src/ui/render/options.rs`: status render configuration.
- `src/ui/layout.rs`: wrapping and cursor placement.
- `src/ui/frame.rs`: frame and line structures.
- `src/ui/span.rs`: styled text spans with wrapping.
- `src/ui/style.rs` + `src/ui/theme.rs`: styles and color theme.

### Terminal
- `src/terminal/terminal.rs`: crossterm integration and rendering primitives.
- `src/terminal/input_event.rs`: internal key events and modifiers.
- `src/terminal/terminal_event.rs`: terminal event enum.

### Entry
- `src/main.rs`: runtime setup and loop.
- `src/lib.rs`: module exports.

## 8. Extension Points

- **New input**: implement `Input`, add rendering and key handling.
- **New component**: implement `Component` and choose `FocusMode`.
- **Focus group**: set `FocusMode::Group` and route keys to children, emitting `ComponentResponse::changes` for input updates.
- **New overlay**: implement `Layer` and swap via `LayerManager`.

---

If you want, I can add diagrams (ASCII flow charts) or split this into smaller docs (e.g., `RENDERING.md`, `FOCUS.md`).
