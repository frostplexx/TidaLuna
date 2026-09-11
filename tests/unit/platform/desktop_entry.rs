//! Tests for `src/platform/desktop_entry.rs`, attached by `#[path]`.

use std::io::{Error, ErrorKind};
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

use super::{Claim, ClaimTarget, WM_CLASS, classify};
use crate::ui::deep_link::SCHEME;

/// The `.deb` entry, pulled in so a rename of either constant has to break here.
/// Nothing derives this file from them; it spells both out by hand.
const DEB_ENTRY: &str = include_str!("../../../installer/linux/deb/tidalunar.desktop.in");
/// The Nix desktop item, hand-written for the same reason.
const NIX_PACKAGE: &str = include_str!("../../../nix/package.nix");

/// `ExitStatus::from_raw` takes a wait status, not an exit code (the code lives
/// in the second byte).
fn exited(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

#[test]
fn the_claim_names_our_own_entry_and_our_own_scheme() {
    // Both halves are the drift guard. Two packaging files spell the scheme out
    // by hand, and a rename here that they miss has to fail somewhere.
    let target = ClaimTarget::ours();
    assert_eq!(target.entry, "tidalunar.desktop");
    assert_eq!(target.mime, "x-scheme-handler/tidal");
}

#[test]
fn a_clean_exit_records_the_claim() {
    assert_eq!(classify(Ok(exited(0))), Claim::Recorded);
}

#[test]
fn a_missing_xdg_mime_is_its_own_outcome() {
    // Not a failure to report loudly, since the machine simply has no way to
    // record a default and the entry is still a candidate.
    let absent = Err(Error::from(ErrorKind::NotFound));
    assert_eq!(classify(absent), Claim::ToolMissing);
}

#[test]
fn a_nonzero_exit_is_a_refusal_and_carries_its_code() {
    assert_eq!(classify(Ok(exited(1))), Claim::Refused(Some(1)));
}

#[test]
fn a_spawn_failure_is_not_a_refusal() {
    // The distinction the outcome exists for. Nothing was asked, so nothing
    // said no.
    let denied = Err(Error::from(ErrorKind::PermissionDenied));
    assert_eq!(classify(denied), Claim::NotRun(ErrorKind::PermissionDenied));
}

#[test]
fn the_deb_entry_declares_the_scheme_and_class_the_code_uses() {
    // The name it lands under is chosen in `xtask` rather than in the file,
    // leaving that third copy of the id unchecked by this.
    let mime = format!("MimeType=x-scheme-handler/{SCHEME};");
    let class = format!("StartupWMClass={WM_CLASS}");
    assert!(DEB_ENTRY.contains(&mime), "the .deb entry has no {mime}");
    assert!(DEB_ENTRY.contains(&class), "the .deb entry has no {class}");
}

#[test]
fn the_nix_desktop_item_names_the_entry_we_claim() {
    // `name` decides what the generated file is called, so it is the id that
    // `ClaimTarget::ours` hands to xdg-mime on a Nix install. A substring is as
    // far as this goes; the test reads Nix source rather than evaluating it.
    let name = format!("name = \"{WM_CLASS}\"");
    let mime = format!("\"x-scheme-handler/{SCHEME}\"");
    assert!(NIX_PACKAGE.contains(&name), "the Nix item has no {name}");
    assert!(NIX_PACKAGE.contains(&mime), "the Nix item has no {mime}");
}
