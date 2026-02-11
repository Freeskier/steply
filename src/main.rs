use steply_v2::app::runtime::Runtime;
use steply_v2::state::app_state::AppState;
use steply_v2::state::demo::build_demo_flow;
use steply_v2::terminal::terminal::Terminal;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
    }
}

fn run() -> std::io::Result<()> {
    let flow = build_demo_flow();
    let state = AppState::new(flow);
    let terminal = Terminal::new()?;
    let mut runtime = Runtime::new(state, terminal);
    runtime.run()
}
