//! Native folder dialog through `rfd` (feature `dialog`).

/// Open the folder dialog on its own thread. `measure-windows.ps1` closes it externally
/// (`WM_CLOSE`), so the run needs no human click and the spike needs no `unsafe`.
pub(crate) fn spawn() {
    std::thread::spawn(|| {
        let r = rfd::FileDialog::new().pick_folder();
        out!("RESULT dialog_returned picked={}", r.is_some());
    });
}
