use steply_v2::runtime::Runtime;
use steply_v2::state::app::AppState;
use steply_v2::state::demo::{build_demo_flow, build_demo_tasks};
use steply_v2::terminal::Terminal;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
    }
}

fn run() -> std::io::Result<()> {
    let flow = build_demo_flow();
    let (task_specs, task_subscriptions) = build_demo_tasks();
    let state = AppState::with_tasks(flow, task_specs, task_subscriptions);
    let terminal = Terminal::new()?.with_mode(steply_v2::terminal::RenderMode::Inline);
    let mut runtime = Runtime::new(state, terminal);
    runtime.run()
}
