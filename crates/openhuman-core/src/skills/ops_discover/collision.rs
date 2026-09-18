//! Compatibility helper for scope precedence tests.

use crate::skills::ops_types::WorkflowScope;

pub(super) const fn precedence(scope: WorkflowScope) -> u8 {
    match scope {
        WorkflowScope::Builtin => 0,
        WorkflowScope::Legacy => 1,
        WorkflowScope::User => 2,
        WorkflowScope::Project => 3,
        WorkflowScope::Profile => 4,
        WorkflowScope::Flow => 5,
    }
}
