//! Stand-in when the `dialog` feature is off: the dialog code is not linked.

/// Report that the build has no dialog.
pub(crate) fn spawn() {
    out!("RESULT error built_without_dialog");
}
