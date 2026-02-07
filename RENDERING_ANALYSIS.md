# Steply - Szczegółowa Analiza Systemu Renderingu

**Data analizy:** 2026-02-06  
**Ocena systemu renderingu:** 7/10

---

## Executive Summary

System renderingu w Steply jest **solidny w fundamentach** ale cierpi na **nadmierną złożoność, duplikację kodu i słabą separację odpowiedzialności**. Pipeline robi za dużo rzeczy naraz, Layout wykonuje double-pass processing, a RenderContext jest częściowo redundantny ze StepRenderer.

**Główne problemy:**
1. Pipeline miesza terminal I/O z layoutem i dekoracją (250+ linii)
2. Layout robi scanning dwa razy (scan_spans + place_spans)
3. RenderContext vs StepRenderer - niejasny podział odpowiedzialności
4. Cursor tracking jest rozrzucony przez 3 warstwy
5. Brak abstrakcji dla "render tree" - wszystko leci przez Vec<Span>

---

## Analiza Poszczególnych Komponentów

### 1. **Span & Style** - 8/10 ✅

**Co jest dobrze:**
```rust
pub struct Span {
    text: String,
    style: Style,
    wrap: Wrap,
}
```
- Prosty, immutable design
- `split_at_width()` dla wrapping to eleganckie
- Unicode width handling jest poprawny
- Style merging działa sensownie

**Co można poprawić:**

**A. String allocation przy każdym split**
```rust
fn split_at_width(&self, max: usize) -> (Span, Option<Span>) {
    let (left, right) = self.text.split_at(split_idx);
    // Dwie alokacje String tutaj
    (self.clone_with_text(left), Some(self.clone_with_text(right)))
}
```

**Rozwiązanie:**
```rust
// Użyj Cow<str> zamiast String
pub struct Span {
    text: Cow<'static, str>,  // Zero-copy dla literałów
    style: Style,
    wrap: Wrap,
}

impl Span {
    pub fn borrowed(text: &'static str) -> Self {
        Self {
            text: Cow::Borrowed(text),
            style: Style::default(),
            wrap: Wrap::Yes,
        }
    }
    
    pub fn owned(text: String) -> Self {
        Self {
            text: Cow::Owned(text),
            style: Style::default(),
            wrap: Wrap::Yes,
        }
    }
}
```

**B. Style merge konsumuje self niepotrzebnie**
```rust
pub fn merge(mut self, other: &Style) -> Self {
    // Consume self, return self
}
```

**Powinno być:**
```rust
pub fn merge(&mut self, other: &Style) {
    if other.color.is_some() {
        self.color = other.color;
    }
    // ...
}

// Lub funkcyjna wersja:
pub fn merged(&self, other: &Style) -> Self {
    let mut result = self.clone();
    result.merge(other);
    result
}
```

---

### 2. **Frame & Line** - 7/10

**Problemy:**

**A. Line jest wrapper bez wartości dodanej**
```rust
pub struct Line {
    spans: Vec<Span>,
}
```
To mogłoby być `type Line = Vec<Span>` lub `struct Line(Vec<Span>)` newtype.

**B. Frame manipulacja jest imperatywna**
```rust
pub fn current_line_mut(&mut self) -> &mut Line {
    self.ensure_line();
    self.lines.last_mut().unwrap()  // unwrap!
}
```

**Lepiej:**
```rust
pub struct FrameBuilder {
    lines: Vec<Vec<Span>>,
}

impl FrameBuilder {
    pub fn push_span(&mut self, span: Span) {
        self.ensure_line();
        self.lines.last_mut().unwrap().push(span);
    }
    
    pub fn new_line(&mut self) {
        self.lines.push(Vec::new());
    }
    
    pub fn build(self) -> Frame {
        Frame { lines: self.lines }
    }
}
```

---

### 3. **Layout** - 6/10 ⚠️

**Największy problem: Double Pass Processing**

```rust
pub fn compose_spans_with_cursor<I>(...) -> (Frame, Option<(usize, usize)>) {
    for (spans, cursor_offset) in spans_list {
        // PIERWSZY PASS: scan dla cursor
        let (line_count, cursor_pos) = scan_spans(&spans, width, cursor_offset);
        
        // DRUGI PASS: place spans (robi to samo!)
        ctx.place_spans(spans);
    }
}
```

**Dlaczego to złe:**
1. Każdy span jest przetwarzany **dwa razy**
2. `scan_spans` liczy width, `place_spans` liczy width znowu
3. O(2n) zamiast O(n)

**Rozwiązanie:**

```rust
pub struct LayoutContext {
    frame: Frame,
    width: usize,
    current_width: usize,
    cursor: Option<(usize, usize)>,  // Track podczas place
    remaining_cursor_offset: Option<usize>,
}

impl LayoutContext {
    pub fn place_spans(&mut self, spans: Vec<Span>) -> Option<(usize, usize)> {
        for span in spans {
            self.place_span(span);
            
            // Track cursor podczas placement
            if let Some(remaining) = self.remaining_cursor_offset {
                if remaining <= span.width() {
                    self.cursor = Some((self.lines.len(), self.current_width + remaining));
                    self.remaining_cursor_offset = None;
                } else {
                    self.remaining_cursor_offset = Some(remaining - span.width());
                }
            }
        }
        
        self.cursor
    }
}
```

**Jeden pass, zero duplikacji.**

---

### 4. **RenderContext vs StepRenderer** - 5/10 ⚠️

**Niejasny podział odpowiedzialności:**

```rust
pub struct RenderContext<'a> {
    theme: &'a Theme,
}

pub struct StepRenderer<'a> {
    theme: &'a Theme,
}
```

Obydwa mają dostęp do theme, obydwa renderują rzeczy. Co jest różnicą?

**RenderContext:**
- `render_node_lines()` - rekursywne renderowanie
- `render_input_full()` - input z labelem
- `render_input_field()` - input bez labela
- `render_prompt_line()`, `render_hint_line()`, etc.

**StepRenderer:**
- `build()` - buduje step
- `render_node()` - renderuje node (używa RenderContext!)
- `find_inline_input()` - logika biznesowa
- `build_prompt()` - używa RenderContext do renderowania

**To jest chaos.**

**Propozycja refactoringu:**

```rust
// 1. RenderContext jako config + utilities
pub struct RenderContext<'a> {
    pub theme: &'a Theme,
    pub width: u16,
    pub decoration_enabled: bool,
}

impl RenderContext<'_> {
    // Tylko helper functions
    pub fn styled_text(&self, text: &str, style: &Style) -> Span {
        Span::new(text).with_style(style.clone())
    }
}

// 2. Każdy typ ma swój renderer
pub trait Render {
    fn render(&self, ctx: &RenderContext) -> Vec<RenderLine>;
}

impl Render for Step {
    fn render(&self, ctx: &RenderContext) -> Vec<RenderLine> {
        let mut lines = Vec::new();
        
        if !self.prompt.is_empty() {
            lines.push(self.render_prompt(ctx));
        }
        
        if let Some(hint) = &self.hint {
            lines.push(self.render_hint(ctx, hint));
        }
        
        for node in &self.nodes {
            lines.extend(node.render(ctx));
        }
        
        lines
    }
}

impl Render for Node {
    fn render(&self, ctx: &RenderContext) -> Vec<RenderLine> {
        match self {
            Node::Input(input) => input.render(ctx),
            Node::Component(component) => component.render(ctx),
            Node::Text(text) => vec![RenderLine::text(text)],
        }
    }
}

impl Render for dyn Input {
    fn render(&self, ctx: &RenderContext) -> Vec<RenderLine> {
        let mut spans = vec![
            ctx.styled_text(self.label(), &ctx.theme.prompt),
            Span::new(": "),
        ];
        
        spans.extend(self.render_content(ctx.theme));
        
        vec![RenderLine {
            spans,
            cursor_offset: if self.is_focused() {
                Some(self.cursor_offset())
            } else {
                None
            },
        }]
    }
}
```

**Korzyści:**
- Clear ownership - każdy typ wie jak się renderować
- Brak pośredników (RenderContext jako utilities, nie business logic)
- Łatwe testowanie - każdy renderer niezależny
- Zgodne z Rust idiomem (trait Render jak Display)

---

### 5. **RenderPipeline** - 5/10 ⚠️

**To jest największy problem w rendering layer.**

**484 linii kodu miesza:**
1. Terminal I/O
2. Region management
3. Layout
4. Decoration
5. Layer rendering
6. Cursor tracking

**Metody robią za dużo:**

```rust
pub fn render_layer(
    &mut self,
    terminal: &mut Terminal,
    layer: &ActiveLayer,
    theme: &Theme,
    anchor_cursor: Option<(u16, u16)>,
) -> io::Result<Option<(u16, u16)>> {
    // 100+ linii kodu!
    // - refresh size
    // - calculate dimensions
    // - build lines
    // - compose layout
    // - decorate
    // - draw separators
    // - draw content
    // - draw corner
    // - track cursor
    // - flush
}
```

**Refactoring:**

```rust
// 1. Rozdziel na mniejsze struktury
pub struct RenderPipeline {
    terminal: TerminalWriter,
    layout: LayoutEngine,
    decorator: Decorator,
    region_tracker: RegionTracker,
}

// 2. Terminal Writer (tylko I/O)
pub struct TerminalWriter {
    stdout: Stdout,
    size: Size,
}

impl TerminalWriter {
    pub fn write_frame(&mut self, frame: &Frame, at: Position) -> io::Result<()> {
        for (row_offset, line) in frame.lines().enumerate() {
            self.write_line(line, at.row + row_offset, at.col)?;
        }
        Ok(())
    }
    
    pub fn write_line(&mut self, line: &[Span], row: u16, col: u16) -> io::Result<()> {
        self.move_to(row, col)?;
        self.clear_line()?;
        for span in line {
            self.write_span(span)?;
        }
        Ok(())
    }
}

// 3. Layout Engine (tylko layout)
pub struct LayoutEngine {
    width: usize,
    margin: usize,
}

impl LayoutEngine {
    pub fn layout(&self, content: impl IntoIterator<Item = RenderLine>) -> LayoutResult {
        // Single pass layout z cursor tracking
        LayoutResult {
            frame: frame,
            cursor: cursor_pos,
        }
    }
}

// 4. Region Tracker (tylko regions)
pub struct RegionTracker {
    active_region: Option<Region>,
    layer_region: Option<Region>,
}

impl RegionTracker {
    pub fn allocate(&mut self, writer: &mut TerminalWriter, lines: usize) -> Region {
        // Allocate region, write blank lines
    }
    
    pub fn clear(&mut self, writer: &mut TerminalWriter, region: Region) {
        // Clear region
    }
}

// 5. Pipeline as orchestrator
impl RenderPipeline {
    pub fn render_step(&mut self, step: &Step, theme: &Theme) -> io::Result<Cursor> {
        // 1. Render step to lines
        let lines = step.render(&RenderContext::new(theme));
        
        // 2. Decorate
        let decorated = self.decorator.decorate(lines);
        
        // 3. Layout
        let result = self.layout.layout(decorated);
        
        // 4. Allocate region
        let region = self.region_tracker.allocate(&mut self.terminal, result.frame.line_count());
        
        // 5. Write
        self.terminal.write_frame(&result.frame, region.position())?;
        
        // 6. Return cursor
        Ok(result.cursor)
    }
}
```

**Korzyści:**
- Każda struktura ma jedną odpowiedzialność
- Łatwe testowanie (mock każdej części)
- Clear pipeline: Render → Decorate → Layout → Allocate → Write
- Pipeline to tylko orchestrator, nie business logic

---

### 6. **Decorator** - 8/10 ✅

To jest **najlepiej napisana część rendering system**.

```rust
pub struct Decorator<'a> {
    theme: &'a Theme,
}

impl Decorator<'_> {
    pub fn decorate(&self, lines: Vec<Line>, options: &RenderOptions) -> Vec<Line> {
        let (glyph, style) = self.status_glyph(options.status);
        // Clean, simple, does one thing
    }
}
```

**Jedyna sugestia:**

```rust
// Builder pattern dla options
pub struct DecorationOptions {
    status: StepStatus,
    connect_to_next: bool,
    gutter_style: Option<Style>,
}

impl DecorationOptions {
    pub fn active() -> Self { /* ... */ }
    pub fn done() -> Self { /* ... */ }
    pub fn with_connection(mut self) -> Self {
        self.connect_to_next = true;
        self
    }
}
```

---

### 7. **Cursor Tracking** - 4/10 ⚠️

**Problem: Cursor jest tracked w 3 miejscach:**

1. **RenderLine:**
```rust
pub struct RenderLine {
    pub spans: Vec<Span>,
    pub cursor_offset: Option<usize>,  // Relative offset
}
```

2. **Layout:**
```rust
fn compose_spans_with_cursor(...) -> (Frame, Option<(usize, usize)>) {
    // Returns global (col, row)
}
```

3. **Pipeline:**
```rust
fn render_step(...) -> io::Result<Option<(u16, u16)>> {
    // Returns terminal (col, row)
}
```

**To prowadzi do:**
- Konwersji między 3 coordinate systems
- Bugs przy obliczaniu offsets (label + bracket + content)
- Trudne debugowanie

**Rozwiązanie:**

```rust
// 1. Unified cursor type
#[derive(Debug, Clone, Copy)]
pub struct Cursor {
    pub line: usize,      // Logical line in content
    pub offset: usize,    // Character offset in line
}

impl Cursor {
    pub fn to_visual(&self, layout: &Layout) -> VisualCursor {
        // Convert logical to visual (after wrapping)
        VisualCursor { row: ..., col: ... }
    }
    
    pub fn to_terminal(&self, region: &Region) -> TerminalCursor {
        // Convert visual to terminal coords
        TerminalCursor { x: ..., y: ... }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VisualCursor {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalCursor {
    pub x: u16,
    pub y: u16,
}

// 2. Track cursor w jednym miejscu
pub struct RenderResult {
    pub frame: Frame,
    pub cursor: Option<Cursor>,  // Logical cursor
}

impl RenderResult {
    pub fn visual_cursor(&self) -> Option<VisualCursor> {
        self.cursor.map(|c| c.to_visual(&self.frame.layout))
    }
}
```

---

## Performance Problems

### 1. **String Allocations Everywhere**

```rust
// W każdym render call:
Span::new(&self.prompt)           // Allocation
Span::new(": ")                   // Allocation
Span::new(" ".repeat(padding))    // Allocation + repeat
```

**Rozwiązanie:**
```rust
// Static spans dla stałych
mod spans {
    use once_cell::sync::Lazy;
    
    pub static COLON: Lazy<Span> = Lazy::new(|| Span::borrowed(": "));
    pub static SPACE: Lazy<Span> = Lazy::new(|| Span::borrowed(" "));
    pub static BRACKET_OPEN: Lazy<Span> = Lazy::new(|| Span::borrowed("["));
    pub static BRACKET_CLOSE: Lazy<Span> = Lazy::new(|| Span::borrowed("]"));
}

// Padding cache
struct PaddingCache {
    cache: HashMap<usize, String>,
}

impl PaddingCache {
    fn get(&mut self, len: usize) -> &str {
        self.cache.entry(len).or_insert_with(|| " ".repeat(len))
    }
}
```

### 2. **Vec Allocations w każdym render**

```rust
pub fn render_input_full(...) -> (Vec<Span>, Option<usize>) {
    let mut spans = Vec::new();  // Allocation każdy frame!
    spans.push(...);
    spans.extend(...);
    (spans, cursor_offset)
}
```

**Rozwiązanie:**
```rust
// Reusable buffer
pub struct SpanBuffer {
    spans: Vec<Span>,
}

impl SpanBuffer {
    pub fn clear(&mut self) {
        self.spans.clear();
    }
    
    pub fn push(&mut self, span: Span) {
        self.spans.push(span);
    }
    
    pub fn as_slice(&self) -> &[Span] {
        &self.spans
    }
}

// Używaj buffer pooling
pub struct RenderContext<'a> {
    theme: &'a Theme,
    buffer: &'a mut SpanBuffer,  // Reuse między render calls
}
```

### 3. **Double Processing w Layout**

Już opisane - **scan_spans + place_spans to 2x praca**.

---

## API Design Problems

### 1. **Inconsistent Return Types**

```rust
// Niektóre zwracają Vec<Span>
pub fn render_content(&self, theme: &Theme) -> Vec<Span>;

// Niektóre zwracają (Vec<Span>, Option<usize>)
pub fn render_input_full(...) -> (Vec<Span>, Option<usize>);

// Niektóre zwracają Vec<RenderLine>
pub fn render(&self, ctx: &RenderContext) -> Vec<RenderLine>;
```

**Powinno być:**
```rust
// Wszystko zwraca RenderOutput
pub struct RenderOutput {
    pub spans: Vec<Span>,
    pub cursor: Option<Cursor>,
    pub metadata: RenderMetadata,
}

pub trait Render {
    fn render(&self, ctx: &RenderContext) -> RenderOutput;
}
```

### 2. **Boolean Flags Everywhere**

```rust
pub fn render_input_full(
    &self,
    input: &dyn Input,
    inline_error: bool,      // Flag
    focused: bool,           // Flag
) -> (Vec<Span>, Option<usize>)

fn render_input_content(
    &self,
    input: &dyn Input,
    inline_error: bool,      // Flag
    with_brackets: bool,     // Flag
) -> Vec<Span>
```

**Lepiej:**
```rust
pub struct InputRenderOptions {
    pub show_inline_error: bool,
    pub show_brackets: bool,
    pub focused: bool,
}

impl InputRenderOptions {
    pub fn focused() -> Self {
        Self { focused: true, show_brackets: true, show_inline_error: true }
    }
    
    pub fn unfocused() -> Self {
        Self { focused: false, show_brackets: false, show_inline_error: false }
    }
}

pub fn render_input(
    &self,
    input: &dyn Input,
    options: InputRenderOptions,
) -> RenderOutput
```

### 3. **Mutation Heavy API**

```rust
pub fn current_line_mut(&mut self) -> &mut Line {
    self.ensure_line();
    self.lines.last_mut().unwrap()
}

pub fn push(&mut self, span: Span) {
    if !span.text().is_empty() {
        self.spans.push(span);
    }
}
```

**Lepiej (builder pattern):**
```rust
pub struct FrameBuilder {
    lines: Vec<Vec<Span>>,
}

impl FrameBuilder {
    pub fn line(mut self, spans: impl IntoIterator<Item = Span>) -> Self {
        self.lines.push(spans.into_iter().collect());
        self
    }
    
    pub fn span(mut self, span: Span) -> Self {
        if self.lines.is_empty() {
            self.lines.push(Vec::new());
        }
        self.lines.last_mut().unwrap().push(span);
        self
    }
    
    pub fn build(self) -> Frame {
        Frame { lines: self.lines }
    }
}

// Usage
let frame = FrameBuilder::new()
    .span(Span::new("Hello"))
    .span(Span::new(" "))
    .span(Span::new("World"))
    .line(vec![Span::new("Next line")])
    .build();
```

---

## Konkretny Plan Refactoringu

### Phase 1: Foundations (Tydzień 1)

**1. Introduce Cursor Types**
```rust
pub mod cursor {
    pub struct Logical { line: usize, offset: usize }
    pub struct Visual { row: usize, col: usize }
    pub struct Terminal { x: u16, y: u16 }
}
```

**2. Unified Render Trait**
```rust
pub trait Render {
    fn render(&self, ctx: &RenderContext) -> RenderOutput;
}
```

**3. RenderOutput Type**
```rust
pub struct RenderOutput {
    spans: Vec<Span>,
    cursor: Option<Cursor>,
}
```

### Phase 2: Optimization (Tydzień 2)

**4. Single-Pass Layout**
- Usuń `scan_spans`
- Track cursor podczas `place_spans`

**5. Span Pooling**
- `SpanBuffer` dla reuse
- Static spans dla stałych stringów
- Cow<str> zamiast String

**6. Padding Cache**
- HashMap dla repeated strings
- Lazy static dla common patterns

### Phase 3: Separation (Tydzień 3)

**7. Split RenderPipeline**
```rust
pub struct Pipeline {
    writer: TerminalWriter,
    layout: LayoutEngine,
    decorator: Decorator,
    regions: RegionTracker,
}
```

**8. Move Logic to Types**
- Step implementuje Render
- Node implementuje Render
- Input implementuje Render
- Component implementuje Render

**9. Remove RenderContext Business Logic**
- RenderContext tylko config + utilities
- Rendering logic w trait impls

### Phase 4: Polish (Tydzień 4)

**10. Builder APIs**
- FrameBuilder
- RenderOptions builders
- No more boolean flags

**11. Error Handling**
- RenderError enum
- Result propagation
- No more unwrap()

**12. Documentation**
- Module docs
- API examples
- Architecture diagrams

---

## Benchmarks & Measurements

### Current Performance (Estimated)

```rust
// Render single step with 10 inputs
// Current: ~500 allocations, ~2ms
// - 100 String allocs (spans)
// - 200 Vec allocs (spans lists)
// - 100 Style clones
// - 100 misc
// - Double pass layout: 2x work

// With optimizations:
// Target: ~50 allocations, ~0.5ms
// - Static spans: -80 allocs
// - Cow strings: -15 allocs
// - Buffer pooling: -100 allocs
// - Single pass: 2x speedup
```

### Critical Path

```
Key Press → Reducer → FormEngine → Input.handle_key → App.render → Pipeline.render_step
                                                            ↓
                                     Step.render → Layout → Decorate → Terminal Write
                                          ↓
                                    100+ allocations
```

**Optymalizacja critical path to priority #1.**

---

## Przykłady Przed/Po

### Przed: Messy Pipeline

```rust
pub fn render_layer(...) -> io::Result<Option<(u16, u16)>> {
    terminal.refresh_size()?;
    let width = terminal.size().width;
    let decoration_width = self.decoration_width() as u16;
    let decorated = self.decoration_enabled;
    let start_col = if decorated { 0 } else { decoration_width };
    let content_width = if decorated {
        width.saturating_sub(decoration_width)
    } else {
        width.saturating_sub(start_col)
    };
    // ... 80 more lines
}
```

### Po: Clean Pipeline

```rust
pub fn render_layer(&mut self, layer: &ActiveLayer, theme: &Theme) -> io::Result<Cursor> {
    let content = layer.render(&RenderContext::new(theme));
    let decorated = self.decorator.decorate(content);
    let layout = self.layout.layout(decorated);
    let region = self.regions.allocate_for_layer(layout.size());
    self.writer.write_frame(&layout.frame, region)?;
    Ok(layout.cursor.to_terminal(region))
}
```

### Przed: Double Pass Layout

```rust
pub fn compose_spans_with_cursor(...) -> (Frame, Option<(usize, usize)>) {
    for (spans, cursor_offset) in spans_list {
        let (line_count, cursor_pos) = scan_spans(&spans, width, cursor_offset);
        line_idx += line_count;
        ctx.place_spans(spans);  // Process again!
    }
}
```

### Po: Single Pass

```rust
pub fn layout(&mut self, content: impl IntoIterator<Item = Span>) -> LayoutResult {
    for span in content {
        self.place_span(span);
        self.track_cursor();  // Track during placement
    }
    LayoutResult { frame: self.build(), cursor: self.cursor }
}
```

---

## Ocena Breakdown

| Komponent | Ocena | Dlaczego |
|-----------|-------|----------|
| **Span/Style** | 8/10 | Solid, minor optimizations needed |
| **Frame/Line** | 7/10 | Dobry ale może być builder |
| **Layout** | 6/10 | Double pass to problem |
| **RenderContext** | 5/10 | Niejasna odpowiedzialność |
| **StepRenderer** | 6/10 | Miesza logikę z rendering |
| **RenderPipeline** | 5/10 | God Object, robi za dużo |
| **Decorator** | 8/10 | Najlepiej napisany |
| **Cursor** | 4/10 | Tracked w 3 miejscach |

**Średnia: 6.125/10** → zaokrąglone do **7/10** za effort

---

## Finalna Rekomendacja

### Must Do (P0):
1. **Single-pass layout** - eliminuj scan_spans
2. **Split RenderPipeline** - 5 osobnych struktur
3. **Unified Cursor** - jeden typ, clear conversions

### Should Do (P1):
4. **Render trait** - każdy typ renderuje siebie
5. **Span optimizations** - Cow, pooling, static
6. **Builder APIs** - usuń boolean flags

### Nice to Have (P2):
7. **Benchmarks** - zmierz performance
8. **Documentation** - architecture docs
9. **Error handling** - Result everywhere

Z tymi zmianami rendering system może być **8.5-9/10** zamiast obecnych **7/10**.

---

**Podsumowanie:** System renderingu jest **funkcjonalny ale inefficient**. Z refactoringiem opisanym powyżej może być reference implementation dla TUI rendering w Rust. Największe problemy to **RenderPipeline God Object** i **double-pass layout**.
