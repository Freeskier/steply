# Steply v2 - pelny opis architektury i przeplywu aplikacji

Ten dokument opisuje **caly aktualny program** end-to-end: od uruchomienia procesu, przez event loop, reducer, stan aplikacji, widgety, renderowanie i terminal, az po walidacje i scheduler.

## 1. Cel aplikacji

Aplikacja to interaktywny TUI (terminal UI) oparty o:
- wieloetapowy flow (`Flow` + `Step`),
- drzewo node'ow (`Input`, `Component`, `Output`),
- model efektow i eventow (`Command` -> `Effect` -> `WidgetEvent`),
- renderer oparty o linie i style (`SpanLine`),
- overlaye (modalne warstwy) z obsluga lifecycle i stosem aktywnych overlay.

## 2. Uruchomienie programu (high level)

Punkt startowy: `src/main.rs`.

1. Budowany jest flow demo: `build_demo_flow()`.
2. Tworzony jest `AppState`.
3. Tworzony jest `Terminal`.
4. Tworzony jest `Runtime`.
5. `Runtime::run()` uruchamia glowna petle aplikacji.

## 3. Struktura warstw

- `src/core`: typy domenowe i reducer.
- `src/runtime`: event loop, komendy, mapowanie klawiszy, scheduler.
- `src/state`: glowny stan aplikacji i logika domenowa flow/focus/overlay/validation.
- `src/widgets`: kontrakty node'ow i implementacje widgetow.
- `src/ui`: layout i renderer.
- `src/terminal`: adapter do crossterm, I/O terminala.

## 4. Core

### 4.1 `NodeId`
Plik: `src/core/mod.rs`

`NodeId` to typed wrapper na `String` (newtype), uzywany zamiast surowych stringow w eventach i stanie.
Daje lepsza spojnosc typow i mniej literowek.

### 4.2 `Value`
Plik: `src/core/value.rs`

Wspolny typ danych dla widgetow:
- `None`,
- `Text(String)`,
- `Bool(bool)`,
- `Number(i64)`,
- `List(Vec<String>)`.

### 4.3 `Reducer`
Plik: `src/core/reducer.rs`

Reducer przyjmuje `Command` i mutuje `AppState`, zwracajac `Vec<Effect>`:
- `Effect::EmitWidget(...)`,
- `Effect::Schedule(...)`,
- `Effect::RequestRender`.

To glowny dispatcher zachowan sterowanych komenda.

## 5. Runtime

### 5.1 Komendy
Plik: `src/runtime/command.rs`

Najwazniejsze komendy:
- `Exit`, `Submit`, `NextFocus`, `PrevFocus`,
- `InputKey`, `TextAction`,
- `OpenOverlay(NodeId)`,
- `OpenOverlayAtIndex(usize)`,
- `OpenOverlayShortcut`,
- `CloseOverlay`,
- `Tick`.

### 5.2 Eventy
Plik: `src/runtime/event.rs`

- `AppEvent`: poziom runtime (`Terminal`, `Command`, `Widget`).
- `WidgetEvent`: poziom UI/widgetow (`ValueProduced`, `RequestFocus`, `OpenOverlay`, lifecycle overlay itd.).

Lifecycle overlay:
- `BeforeOpen`, `Opened`,
- `BeforeClose`, `Closed`, `AfterClose`.

### 5.3 Key bindings
Plik: `src/runtime/key_bindings.rs`

Domyslne mapowania:
- `Ctrl+C` -> exit,
- `Ctrl+O` -> `OpenOverlayShortcut`,
- `Ctrl+1/2/3` -> `OpenOverlayAtIndex(0/1/2)`,
- `Alt+1/2/3` -> `OpenOverlayAtIndex(0/1/2)`,
- `Esc` -> exit/close overlay (przez reducer + state),
- `Tab` / `Shift+Tab` -> `NextFocus` / `PrevFocus` (w reducerze: najpierw completion, potem fallback do focus navigation),
- `Ctrl+Backspace`, `Ctrl+W`, `Ctrl+Delete` -> text actions.

### 5.4 Runtime loop
Plik: `src/runtime/runner.rs`

Petla wykonania:
1. `terminal.enter()`
2. pierwsze `render()`
3. while `!state.should_exit()`:
   - obsluga scheduler (`drain_ready`),
   - poll eventu terminala,
   - dispatch eventu,
   - ewentualny rerender
4. `terminal.exit()`

### 5.5 Scheduler
Plik: `src/runtime/scheduler.rs`

Scheduler obsluguje:
- `EmitNow`,
- `EmitAfter`,
- `Debounce`,
- `Throttle`,
- `Cancel`.

Mechanizm oparty o wersjonowanie kluczy (`key_versions`) i kolejke delayed tasks.

## 6. State (serce logiki)

### 6.1 Flow i Step
Pliki: `src/state/flow.rs`, `src/state/step.rs`

`Flow` przechowuje:
- `steps`,
- `current index`,
- `statuses` (`Pending`, `Active`, `Done`, `Cancelled`).

`Step` ma:
- `id`, `prompt`, `hint`,
- `nodes: Vec<Node>`,
- `validators` (step-level).

### 6.2 Focus
Plik: `src/state/focus.rs`

`FocusState` trzyma liste focus targetow i index aktywnego targetu.
Budowa listy idzie po drzewie node'ow (focusowalne sa `Leaf` i `Group`).

### 6.3 Overlay state
Plik: `src/state/overlay.rs`

`OverlayState` utrzymuje stos aktywnych overlay (`stack: Vec<OverlayEntry>`).
`OverlayEntry` zawiera:
- `id`,
- `mode` (`Exclusive` / `Shared`),
- `focus_mode`,
- `focus_before_open`.

To jest centralny manager overlay (source of truth dla aktywnych warstw).

### 6.4 Value store
Plik: `src/state/store.rs`

Globalny magazyn wartosci node'ow (`HashMap<NodeId, Value>`).
Uzywany do przenoszenia wartosci miedzy krokami i do hydracji nowego stepu.

### 6.5 Validation
Plik: `src/state/validation/mod.rs`

- walidacja per-node (`ValidationEntry`, visibility hidden/inline),
- walidacja step-level (`ValidationIssue::Step` + `step_errors`),
- `ValidationContext` udostepnia wartosci widgetow validatorom kroku.

## 7. AppState - glowna orkiestracja

Pliki:
- `src/state/app_state/mod.rs`
- `src/state/app_state/navigation.rs`
- `src/state/app_state/value_sync.rs`
- `src/state/app_state/validation_runtime.rs`

### 7.1 Co przechowuje `AppState`
- `flow`,
- `overlays` (stack),
- `store`,
- `validation`,
- `pending_scheduler`,
- `focus`,
- `completion_session`,
- `should_exit`.

### 7.2 Wybor aktywnych node'ow
`active_nodes()` i `active_nodes_mut()` decyduja gdzie trafia input:
- jesli brak blocking overlay -> aktywny jest step,
- jesli jest blocking overlay:
  - dla `FocusMode::Group` aktywny scope to step (group sam routuje focus),
  - dla pozostalych mode aktywny scope to dzieci overlay.

### 7.3 Obsluga eventow widget
`handle_widget_event(...)` obsluguje m.in.:
- `ValueProduced`: zapis wartosci + czyszczenie step errors,
- `RequestSubmit`: submit stepu albo close overlay,
- `RequestFocus`: jawna zmiana focusu,
- `OpenOverlay` / `CloseOverlay`.

### 7.4 Otwieranie/zamykanie overlay
`open_overlay_by_id(...)`:
- emituje lifecycle `BeforeOpen` -> `Opened`,
- otwiera overlay,
- dopisuje wpis do `OverlayState` (stack),
- przebudowuje focus.

`close_overlay()`:
- zdejmuje top overlay ze stacka,
- emituje `BeforeClose` -> `Closed` -> `AfterClose`,
- przywraca focus z `focus_before_open`.

### 7.5 Submit stepu
`handle_step_submit()`:
1. walidacja obecnego stepu,
2. sync wartosci stepu do store,
3. `flow.advance()` do kolejnego stepu,
4. hydratacja nowego stepu wartosciami ze store,
5. rebuild focus.

### 7.6 Tick
`tick_all_nodes()` iteruje po **state traversal** calego flow i wywoluje `on_tick()` na kazdym node.

### 7.7 Sync wartosci
`value_sync.rs`:
- `sync_current_step_values_to_store()`,
- `hydrate_current_step_from_store()`,
- `set_value_by_id()` i bezposrednia aplikacja na node w kroku.

### 7.8 Walidacja runtime
`validation_runtime.rs`:
- `validate_focused(...)`,
- `validate_current_step(...)`,
- step validators (`ValidationIssue::Node` i `ValidationIssue::Step`),
- debounce/cancel inline error przez scheduler.

### 7.9 Completion runtime
`navigation.rs` utrzymuje globalny flow completion:
- `handle_tab_forward()` i `handle_tab_backward()` obsluguja `Tab`/`Shift+Tab`,
- najpierw probuja completion na aktualnie fokusowanym node,
- jesli completion nie pasuje: przekazuja `Tab`/`BackTab` do widgetu (`on_key`) - to pozwala np. `Modal(FocusMode::Group)` routowac focus wewnetrzny,
- jesli widget nie obsluzy klawisza, dopiero wtedy uruchamiana jest standardowa nawigacja focus (`focus_next` / `focus_prev`).

Stan sesji completion (`CompletionSession`) przechowuje:
- `owner_id`,
- oryginalny `prefix`,
- liste dopasowan,
- aktualny indeks dopasowania (cyklowanie Tab/Shift+Tab).

## 8. Kontrakt widgetow i drzewo Node

### 8.1 Traits
Plik: `src/widgets/traits.rs`

Glowne kontrakty:
- `Drawable::draw(...) -> DrawOutput`,
- `Interactive`:
  - input (`on_key`, `on_text_action`, `on_event`, `on_tick`),
  - tekst (`text_edit_state`, `after_text_edit`) - domyslna globalna obsluga `TextAction`,
  - completion (`completion_state`) - domyslnie brak completion,
  - value (`value`, `set_value`, `validate`),
  - overlay metadata (`overlay_mode`, `overlay_open/close`, `overlay_placement`),
  - tree API (`children`, `state_children`).

`InteractionResult` niesie:
- `handled`,
- `request_render`,
- `events`.

### 8.2 Node enum
Plik: `src/widgets/node.rs`

`Node` ma warianty:
- `Input(Box<dyn InteractiveNode>)`,
- `Component(Box<dyn InteractiveNode>)`,
- `Output(Box<dyn RenderNode>)`.

Dostarcza:
- forwarding wszystkich metod runtime,
- wyszukiwarki (`find_node`, `find_overlay`, itd.),
- dwa rodzaje traversalu:
  - `visit_nodes` (renderowy),
  - `visit_state_nodes` (stanowy).

## 9. Widgety w projekcie

### 9.1 Input
Plik: `src/widgets/inputs/input.rs`

- focus mode: `Leaf`,
- edycja tekstu i kursor,
- dynamiczna lista completion (`completion_items`) podpieta pod globalny mechanizm `Tab`,
- walidatory lokalne,
- Enter: `ValueProduced` do targetu albo `RequestSubmit`.

### 9.2 FilterSelect
Plik: `src/widgets/components/filter_select.rs`

- focus mode: `Group`,
- laczy query input + pionowa liste opcji,
- strzalki gora/dol wybor,
- completion query dziala na bazie `options` (lista opcji jest kandydatem completion),
- Enter produkuje wybrana wartosc do targetu.

### 9.3 Modal
Plik: `src/widgets/components/modal.rs`

- komponent overlay,
- ma `overlay_mode` (`Exclusive` / `Shared`) i `focus_mode`,
- `Group` ma lokalny routing fokusowania (Tab/BackTab po dzieciach),
- obsluguje lifecycle eventy overlay,
- `children()` zwraca tylko gdy visible (render),
- `state_children()` zwraca zawsze (state traversal).

### 9.4 Text output
Plik: `src/widgets/outputs/text.rs`

Prosty node render-only, jedna linia tekstu.

## 10. Rendering

### 10.1 Renderer bazowy
Plik: `src/ui/renderer.rs`

Renderer:
1. buduje frame krokow (`build_base_frame`),
2. dekoruje kroki (`decorate_step_block`),
3. mapuje kursor po wrap (`Layout::compose_with_cursor`),
4. doklada overlaye ze stacka (bottom -> top).

Status stepu mapuje style i markery:
- Active: zielony/cyan,
- Done/Pending: szary,
- Cancelled: czerwony.

### 10.2 Overlay renderer
Plik: `src/ui/renderer/overlay.rs`

Dla kazdego overlay:
- renderuje content dzieci,
- owija go ramka (`┌ ┐ │ └ ┘`),
- blenduje komorki na bazowym frame,
- mapuje kursor do wspolrzednych overlay.

### 10.3 Layout
Plik: `src/ui/layout.rs`

- sklada linie do szerokosci terminala,
- obsluguje `Wrap` vs `NoWrap`,
- uwzglednia unicode width,
- mapuje kursor z pozycji zrodlowej na po wrapie.

## 11. Terminal backend

Plik: `src/terminal/backend.rs`

Backend oparty o crossterm:
- raw mode + hide/show cursor,
- poll eventow (`Key`, `Resize`, fallback `Tick`),
- render linii ze stylem,
- clipping po unicode width,
- zarzadzanie przestrzenia pionowa (`ScrollUp`) tak, aby nie ucinalo UI przy starcie na dole historii terminala.

## 12. Przeplyw informacji (pelna sciezka)

Przyklad: user naciska klawisz.

1. `Terminal` zwraca `TerminalEvent::Key`.
2. `Runtime` mapuje to przez `KeyBindings` do `Command`.
3. `Reducer` wykonuje logike i produkuje `Effect`.
4. `Runtime` aplikuje efekty:
   - eventy widget -> `AppState::handle_widget_event`,
   - schedule -> `Scheduler`,
   - request render -> `Renderer`.
5. `Renderer` buduje nowy frame i kursor.
6. `Terminal::render` rysuje wynik.

## 13. Aktualny flow demo

Plik: `src/state/demo.rs`.

Step 1:
- outputy z opisem,
- inputy: `tags_raw`, `dupa`,
- `FilterSelect` (`tag_picker`),
- `Modal` (`demo_overlay`) z inputem mirrorujacym do `tags_raw`,
- walidator step-level (np. `tags_raw` != `dupa`).

Step 2:
- output z opisem,
- input `selected_tag`,
- walidator step-level (blokada wartosci `forbidden`).

## 14. Co jest najwazniejsze architektonicznie

1. Jeden centralny stan (`AppState`) + czytelny podzial submodulow.
2. Typed `NodeId`.
3. Oddzielenie traversalu stanu od traversalu renderu.
4. Overlay stack jako dedykowany manager (`OverlayState`).
5. Unifikacja sygnalow interakcji przez `InteractionResult`.
6. Renderer oparty o jawny model linii i kompozycje warstw.

## 15. Ograniczenia i decyzje projektowe

- Render dzieci jest kontrolowany przez rodzica (swiadoma decyzja: duza elastycznosc komponentow).
- Focus lista budowana jest na bazie `children()` (renderowe drzewo), co oznacza ze widocznosc komponentow wplywa na nawigacje fokusowa.
- Dla skrótow overlay indeksowych (`Ctrl/Alt + 1/2/3`) ilosc realnie dostepnych overlay zalezy od tego, ile overlay zdefiniowano w aktualnym kroku.
- Completion dziala tokenowo: podmienia tylko aktualne slowo przy kursorze (dopasowanie `starts_with`, case-insensitive).

## 16. Jak rozszerzac program

Rekomendowany pattern:
1. Dodaj nowy widget implementujac `Drawable + Interactive`.
2. Okresl `focus_mode` i kontrakt eventow (`on_key`, `on_event`, `on_tick`).
3. W razie potrzeby wystaw `children` i/lub `state_children`.
4. Dodaj node do stepu (`state/demo.rs` albo nowy builder flow).
5. Jezeli potrzebujesz logiki globalnej, dodaj `WidgetEvent` i obsluge w `AppState`.

---
Dokument opisuje aktualny stan kodu w `src/` i ma sluzyc jako mapa calej aplikacji do dalszego rozwoju core.
