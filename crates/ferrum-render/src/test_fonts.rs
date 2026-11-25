//! The fixture fonts of `fixtures/fonts`, installed in a temporary directory, as the tests' system
//! fonts (`docs/08` §7: tests never read the runner's or the user's own fonts).

use ferrum_testkit::temp::TempDir;

use crate::font::SystemFonts;

/// The fixture fonts copied into a fresh temporary directory, and their scan. The directory
/// lives as long as the returned guard.
pub(crate) fn fixture_fonts() -> (TempDir, SystemFonts) {
    let dir = TempDir::new("render-fonts").expect("temporary directory");
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts");
    for entry in std::fs::read_dir(&fixtures).expect("fixtures/fonts") {
        let path = entry.expect("fixture entry").path();
        if path.extension().is_some_and(|extension| extension == "ttf") {
            let name = path.file_name().expect("file name");
            std::fs::copy(&path, dir.join(name)).expect("copy a fixture font");
        }
    }
    let fonts = SystemFonts::scan_dir(dir.path());
    (dir, fonts)
}
