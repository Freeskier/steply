pub mod engine;
pub mod execution;
mod inline;
pub mod policy;
pub mod run_state;
pub mod spec;

pub use engine::TaskStartResult;
pub use execution::{TaskCancelToken, TaskCompletion, TaskInvocation, TaskRequest};
pub use policy::{ConcurrencyPolicy, RerunPolicy};
pub use run_state::TaskRunState;
pub use spec::{TaskId, TaskKind, TaskSpec, TaskTrigger};

pub use inline::TaskSetupError;
pub(crate) use inline::{collect_inline_tasks_from_flow, validate_task_id_collisions};
