use steply_v2::app::runtime::Runtime;
use steply_v2::state::app_state::AppState;
use steply_v2::state::demo::build_demo_flow;
use steply_v2::terminal::terminal::Terminal;
use steply_v2::ui::options::RenderOptions;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
    }
}

fn run() -> std::io::Result<()> {
    let flow = build_demo_flow();
    let state = AppState::new(flow);
    let terminal = Terminal::new()?;
    let decorations_enabled = std::env::var("STEPLY_DECORATIONS")
        .map(|value| !matches!(value.as_str(), "0" | "false" | "off" | "no"))
        .unwrap_or(true);
    let render_options = RenderOptions {
        decorations_enabled,
    };
    let mut runtime = Runtime::new(state, terminal).with_render_options(render_options);
    runtime.run()
}
