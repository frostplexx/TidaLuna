//! Tests for `src/platform/url_scheme.rs`, attached by `#[path]`. That file is
//! Windows-only; these run in the Windows pass and are absent from the Linux one.

use std::path::Path;

use super::open_command;

/// Pulled in so a change to either side breaks here. The uninstaller cannot see
/// `open_command`, and a command it stops recognising leaves our own stale key
/// behind, whose only symptom is a `tidal://` click that opens nothing.
const NSI: &str = include_str!("../../../installer/windows/tidalunar.nsi");

#[test]
fn the_open_command_quotes_the_exe_and_takes_the_url_as_its_argument() {
    // Quoting is not decoration, the default install path having a space in it.
    assert_eq!(
        open_command(Path::new(r"C:\Program Files\TidaLunar\tidalunar.exe")),
        r#""C:\Program Files\TidaLunar\tidalunar.exe" "%1""#
    );
}

#[test]
fn the_uninstaller_compares_against_the_command_we_write() {
    let ours = open_command(Path::new(r"$INSTDIR\tidalunar.exe"));
    assert!(
        NSI.contains(&format!("== '{ours}'")),
        "the uninstaller no longer compares against {ours}"
    );
}
