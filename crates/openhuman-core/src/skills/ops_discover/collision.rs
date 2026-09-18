//! Compatibility helper for scope precedence tests.

use crate::skills::ops_types::WorkflowScope;

pub(super) const fn precedence(scope: WorkflowScope) -> u8 {
    scope.precedence()
}
