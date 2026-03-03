use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct HostContext {
    pub cwd: PathBuf,
    pub home_dir: Option<PathBuf>,
}

impl Default for HostContext {
    fn default() -> Self {
        Self {
            cwd: PathBuf::from("/"),
            home_dir: None,
        }
    }
}

static HOST_CONTEXT: OnceLock<HostContext> = OnceLock::new();

pub fn set_host_context(ctx: HostContext) -> Result<(), HostContext> {
    HOST_CONTEXT.set(ctx)
}

pub fn cwd() -> PathBuf {
    HOST_CONTEXT
        .get()
        .map(|ctx| ctx.cwd.clone())
        .unwrap_or_else(|| HostContext::default().cwd)
}

pub fn home_dir() -> Option<PathBuf> {
    HOST_CONTEXT.get().and_then(|ctx| ctx.home_dir.clone())
}
