# Refactor Areas Plan

## Cel
Uporządkować architekturę bez pełnego rewrite, ograniczyć workaroundy i poprawić czytelność kodu przy zachowaniu obecnych feature'ów.

## Priorytety (od najwyższego wpływu)

### 1) `ui/layout` + `ui/renderer`
- Problem:
  - Tu kumuluje się najwięcej wyjątków (wrap/no-wrap, dekoracje stepu, sticky, hinty, spacing).
  - Zmiany wizualne często psują inne przypadki.
- Kierunek:
  - Ujednolicić model układania linii (np. semantyczne tokeny inline zamiast heurystyk).
  - Oddzielić odpowiedzialności: skład treści, dekoracje stepu, sticky sections.
- Efekt:
  - Mniej regresji wizualnych, prostsza logika renderu.

### 2) Focus/Cursor Lifecycle (globalnie)
- Problem:
  - Focus aktywnego stepu/inputu czasem „ucieka”, a widoczność kursora bywa niespójna.
- Kierunek:
  - Jedno źródło prawdy dla:
    - aktywnego widgetu,
    - anchor scrolla,
    - zasad `cursor_visible`.
  - Wyeliminować lokalne wyjątki per-widget tam, gdzie nie są konieczne.
- Efekt:
  - Stabilna nawigacja wstecz/przód i brak „gubienia” kursora.

### 3) Kontrakt `widgets` (API i zachowanie)
- Problem:
  - Różne inputy mają różne, częściowo ukryte zasady obsługi klawiszy i renderowania.
- Kierunek:
  - Dookreślić i spiąć wspólny kontrakt:
    - `draw`,
    - `on_key`,
    - `cursor_visible`,
    - `hints`,
    - `validate`.
  - Standaryzacja Enter/Shift+Enter/Ctrl+W/backspace/tab.
- Efekt:
  - Mniej logiki „specjalnej”, łatwiejsze dodawanie kolejnych komponentów.

### 4) Granice modułów stanu (`state/app`, `task`, `completion`)
- Problem:
  - Nadal występują przecieki odpowiedzialności między modułami.
- Kierunek:
  - Dokończyć separację:
    - UI state,
    - runtime state,
    - task orchestration,
    - completion pipeline.
  - Ustalić jasny kierunek zależności (który moduł może znać który).
- Efekt:
  - Czytelniejsza architektura i mniej efektów ubocznych zmian.

### 5) Kompozycja stepu (content vs chrome)
- Problem:
  - Dekoracje (`│`, `└`, warning/error/help) mieszają się z treścią stepu.
- Kierunek:
  - Rozdzielić:
    - `StepContent` (widgety),
    - `StepChrome` (obramowanie, status, hinty).
- Efekt:
  - Spójne spacingi i przewidywalne reguły renderowania.

### 6) Wspólny core dla list/table/tree/file_browser
- Problem:
  - Powtórzona logika: aktywny index, filtr, scrolling, loading footer.
- Kierunek:
  - Wyciągnąć powtarzalne zachowania do shared core.
  - Komponenty domenowe zostawiają tylko specyficzny rendering.
- Efekt:
  - Redukcja kodu i mniej rozjazdów UX między komponentami.

### 7) Inputy tekstowe (`text`, `textarea`, `masked`, `select`)
- Problem:
  - Niespójne zachowania wrap/edycji i punktowe bugi.
- Kierunek:
  - Wspólna warstwa edycyjna + jednolite zasady wrap.
  - Dopiero na tym cienka warstwa specyficzna per input.
- Efekt:
  - Mniej błędów klasowych i prostsze utrzymanie.

### 8) `completion` pipeline
- Problem:
  - Logika snapshot/apply/lifecycle jest nadal częściowo rozproszona.
- Kierunek:
  - Jedna, czytelna ścieżka danych completion:
    - event -> snapshot -> render -> apply.
- Efekt:
  - Lepsza diagnozowalność i łatwiejszy rozwój.

### 9) File structure i nazewnictwo
- Problem:
  - Część modułów jest logicznie blisko, ale leży daleko w strukturze.
- Kierunek:
  - Dokończyć porządkowanie katalogów i ujednolicić naming (domenowy, nie techniczny).
- Efekt:
  - Krótszy onboarding i łatwiejsza orientacja w kodzie.

### 10) Guardrails architektoniczne
- Problem:
  - Bez twardych reguł łatwo wracają szybkie workaroundy.
- Kierunek:
  - Dodać lekkie zasady (arch rules) i checklistę PR:
    - brak nowych cross-layer zależności,
    - brak „tymczasowych” flag bez daty usunięcia,
    - zmiany renderu muszą mieć scenariusze ręcznej walidacji.
- Efekt:
  - Wolniejsze narastanie długu technicznego.

## Dlaczego nie rewrite od zera
- Aktualny kod ma już dużo działających edge-case’ów.
- Rewrite zwiększa ryzyko regresji i zwykle trwa dłużej niż planowany refactor modułowy.
- Lepszy ROI daje podejście iteracyjne: stabilizacja rdzenia + migracja krok po kroku.

## Proponowana kolejność wdrożenia (3 etapy)

### Etap 1 (Quick Wins, 1-2 tyg.)
1. Ujednolicenie focus/cursor lifecycle.
2. Rozdzielenie StepContent vs StepChrome (bez zmiany feature setu).
3. Standaryzacja hint rendering (jedna ścieżka).

### Etap 2 (Core Cleanup, 2-4 tyg.)
1. Refactor `ui/layout + renderer` do bardziej semantycznego modelu łamania.
2. Ujednolicenie kontraktu widgetów.
3. Wspólny core nawigacji/filtrowania dla list/table/tree/file_browser.

### Etap 3 (Architecture Hardening, 2-3 tyg.)
1. Dokończenie separacji `state/app/task/completion`.
2. Porządki w strukturze plików i nazewnictwie.
3. Wprowadzenie guardrails (reguły architektoniczne + checklista zmian).

## Kryteria sukcesu
- Zmiana w jednym komponencie nie psuje renderu w innych stepach.
- Fokus i cursor zachowują się przewidywalnie przy nawigacji wstecz/przód.
- Redukcja duplikacji w komponentach listowych.
- Mniej warunków specjalnych i flag obejściowych w rendererze.
- Czytelna odpowiedzialność modułów `state`, `runtime`, `ui`, `widgets`.
