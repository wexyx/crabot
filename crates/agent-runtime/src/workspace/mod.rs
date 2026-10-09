mod access;
mod approvals;
mod settings;
pub use access::{OutsideAccess, Workspace};
pub(crate) use approvals::confirm_command;
pub(crate) use approvals::confirm_process_input;
pub use approvals::with_approval_context;
pub use approvals::{ApprovalRequest, decide, pending};
pub use settings::WorkspaceSettings;

pub use approvals::{allow_conversation, revoke_conversation_approval, with_conversation_approval};

mod temporary;
pub(crate) use temporary::temporary_dir;
