# Steply V2 - Status i Plan Dalszych Prac

## 1. Cel V2

Celem V2 jest uproszczenie i uczytelnienie architektury TUI:
- prosty, przewidywalny przepływ eventów,
- centralna orkiestracja stanu,
- modularny system widgetów,
- łatwe dokładanie kolejnych inputów/komponentów bez duplikacji boilerplate.

---

## 2. Co już zrobiliśmy

### 2.1 Fundament aplikacji i runtime

Wdrożono nowy core aplikacji:
- `Runtime` z pętlą eventową,
- `Command -> Reducer -> Effect -> Runtime` jako główny pipeline,
- obsługa `AppEvent` i `WidgetEvent`.

Pliki:
- `v2/src/app/runtime.rs`
- `v2/src/app/command.rs`
- `v2/src/app/event.rs`
- `v2/src/domain/reducer.rs`
- `v2/src/domain/effect.rs`

### 2.2 Scheduler (uniwersalny moduł odroczonych eventów)

Dodano uniwersalny scheduler z obsługą:
- `EmitNow`
- `EmitAfter`
- `Debounce`
- `Throttle`
- `Cancel`

Scheduler jest podpięty do runtime i może obsługiwać:
- timed overlay/error,
- debouncing wyszukiwania,
- opóźnione akcje UI.

Plik:
- `v2/src/app/scheduler.rs`

### 2.3 Terminal + render interaktywny inline

Wdrożono terminal oparty o `crossterm`:
- raw mode,
- key-by-key events,
- resize events,
- render inline (bez czyszczenia historii terminala),
- rysowanie w obszarze pod miejscem uruchomienia,
- obsługa kursora.

Plik:
- `v2/src/terminal/terminal.rs`

### 2.4 Architektura UI: Span + Layout + Style

Dodano lekką wersję warstwy renderującej:
- `Span` + `SpanLine`,
- `WrapMode` (`NoWrap`, `Wrap`),
- `Style` (`color`, `background`),
- `Layout::compose(...)` do łamania linii,
- mapowanie styli do crossterm.

Pliki:
- `v2/src/ui/span.rs`
- `v2/src/ui/style.rs`
- `v2/src/ui/layout.rs`
- `v2/src/ui/renderer.rs`

### 2.5 Node, widgety i capability traits

Zachowaliśmy podejście capability-based:
- `Drawable`
- `Interactive`
- `Node` jako `Input | Component | Output`

Dzięki temu:
- outputy są proste (tylko draw),
- interaktywne elementy mają wejście klawiatury, cursor i value.

Pliki:
- `v2/src/widgets/traits.rs`
- `v2/src/node.rs`

### 2.6 InputBase i ComponentBase

Dodano bazowe struktury redukujące boilerplate:
- `InputBase` (id/label/focus + helpery)
- `ComponentBase` (id/label/focus + helpery)

Przepięte widgety:
- `TextInput`
- `CheckboxInput`
- `GroupComponent`
- `SelectListComponent`

Pliki:
- `v2/src/widgets/base.rs`
- `v2/src/widgets/input_text.rs`
- `v2/src/widgets/checkbox_input.rs`
- `v2/src/widgets/group_component.rs`
- `v2/src/widgets/select_list_component.rs`

### 2.7 Steps + Flow + globalny ValueStore

Wdrożono model kroków i globalnych wartości:
- `Step`
- `Flow` (z `current_step`)
- `ValueStore` (globalne wartości między krokami)

Demo przeniesione do osobnego pliku:
- `build_demo_flow()`

Pliki:
- `v2/src/state/step.rs`
- `v2/src/state/flow.rs`
- `v2/src/state/store.rs`
- `v2/src/state/demo.rs`
- `v2/src/state/app_state.rs`

### 2.8 Layer manager

Wydzielono `LayerManager` dla aktywnej warstwy modal/shared.

Plik:
- `v2/src/state/layer.rs`

### 2.9 Main i bootstrap

`main.rs` ładuje jawnie flow demo i startuje runtime.

Plik:
- `v2/src/main.rs`

---

## 3. Aktualne założenia architektoniczne

1. `Reducer` mutuje stan domenowy, runtime wykonuje efekty.
2. Widgety nie modyfikują globalnego stanu bezpośrednio; emitują eventy.
3. Steps/flow są częścią stanu aplikacji.
4. Globalny store wartości umożliwia przekazywanie danych między krokami.
5. Scheduler obsługuje logikę czasową i odroczone eventy.
6. Render oparty o span/layout/style, a nie tylko surowe stringi.

---

## 4. Uzgodnione podejście do walidacji

Kierunek (uzgodniony):
- walidacja per input,
- możliwość walidacji komponentów,
- polityka walidacji centralnie w reducer/app state,
- reguły walidacji trzymane blisko pól (input/component),
- renderer decyduje o prezentacji błędu (live vs komunikat przy próbie przejścia dalej).

Wniosek projektowy:
- komponenty też mogą implementować walidację (zwłaszcza złożone: table/tree/select/file-browser).

---

## 5. Co planujemy dalej

## 5.1 Walidacja v2 (systemowo, bez duplikacji)

Plan implementacyjny:
1. Dodać kontrakt walidacji dla interaktywnych node’ów (input + opcjonalnie component).
2. Trzymać wynik walidacji centralnie w stanie aplikacji.
3. Na keypress: live validation aktywnego pola.
4. Na Tab/Submit: walidacja blokująca i ujawnienie komunikatu błędu.
5. Użyć schedulera do timed error overlays (np. 2 sekundy) tam, gdzie ma to sens UX.

## 5.2 Binding graph (source -> target)

Obecnie część mapowań jest hardcoded (demo).
Plan:
- wprowadzić jawny graph powiązań `NodeId -> NodeId` / porty,
- przenieść transformacje danych do dedykowanej warstwy bindingu,
- uprościć przepływ między stepami i komponentami.

## 5.3 Rozwój zestawu inputów i komponentów

Priorytet:
1. `DateInput`
2. `MaskedInput`
3. dalsze komponenty z poprzedniej wersji

Podejście:
- nowy input = `InputBase` + własny model wartości + `on_key` + `draw` + `cursor_pos`,
- minimum boilerplate, maksimum logiki domenowej w samym widżecie.

## 5.4 Dalsze uporządkowanie AppState

Plan:
- dalej odchudzać `app_state.rs` przez wydzielanie modułów domenowych:
  - submit/next-step policy,
  - synchronizacja store,
  - hydration stepów,
  - walidacja.

## 5.5 Utrwalenie API i ADR

Plan:
- zapisać kluczowe decyzje jako ADR:
  - capability traits,
  - scheduler as delayed-event engine,
  - flow/step/store ownership,
  - validation policy.

---

## 6. Podejście realizacyjne (jak pracujemy dalej)

Uzgodnione podejście:
1. Iteracyjnie, bez big-bang rewrite.
2. Najpierw stabilizujemy core (runtime/store/validation/scheduler), potem dokładamy feature’y.
3. Każda nowa funkcja ma być osadzona w obecnym modelu (`Reducer + Effect + Scheduler`).
4. Minimalizujemy duplikację przez bazy i helpery (`InputBase`, `ComponentBase`, render helpers).
5. Utrzymujemy prosty mental model i czytelny podział odpowiedzialności między modułami.

---

## 7. Krótki status na teraz

Status: **działający, spójny szkielet V2** z:
- step flow,
- globalnym store,
- schedulerem,
- interaktywnym terminalem inline,
- span/layout/style,
- bazami dla input/component,
- demo pokazującym przepływ danych między krokami.

Następny duży krok: **wdrożenie pełnej walidacji systemowej** zgodnie z powyższym planem.

---

## 8. Zrealizowane w tej iteracji

Wdrożono:
- centralny `ValidationState` w `AppState`,
- kontrakt `validate()` dla interaktywnych node'ów,
- live validation aktywnego pola na keypress,
- blocking validation na zmianie fokusu (`Tab`/`BackTab`) i submit kroku,
- render błędów walidacji przez `Renderer` (linia pod node'em),
- jawny `BindingGraph` (`source -> target`) z transformacjami (`Identity`, `CsvToList`),
- usunięcie hardcodu transformacji `tags_raw` z `AppState` na rzecz bindingów,
- nowy `DateInput` (`YYYY-MM-DD`) oparty o `InputBase`,
- ADR-y w `v2/ADR/`:
  - `0001-capability-traits.md`
  - `0002-scheduler-delayed-events.md`
  - `0003-flow-step-store-ownership.md`
  - `0004-validation-policy.md`
