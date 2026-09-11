//! Tests for `src/ui/deep_link.rs`, attached to it by `#[path]`.

use super::*;

fn target(raw: &str) -> String {
    target_for(raw)
        .unwrap_or_else(|| panic!("expected {raw} to be accepted"))
        .to_string()
}

/// What the renderer is handed, which is not the same string as the one above,
/// since the escapes this file guards against all live in the path alone.
fn route(raw: &str) -> String {
    target_for(raw)
        .unwrap_or_else(|| panic!("expected {raw} to be accepted"))
        .route()
        .to_owned()
}

#[test]
fn routes_a_content_path_onto_our_own_origin() {
    assert_eq!(
        target("tidal://album/175122451"),
        "https://desktop.tidal.com/album/175122451"
    );
    assert_eq!(
        target("tidal://track/9?foo=bar"),
        "https://desktop.tidal.com/track/9?foo=bar"
    );
}

#[test]
fn accepts_the_scheme_in_any_case() {
    // KDE and the Windows shell hand back whatever case the emitter wrote.
    assert_eq!(
        target("TIDAL://album/1"),
        "https://desktop.tidal.com/album/1"
    );
}

#[test]
fn an_empty_path_lands_on_the_home_page() {
    assert_eq!(target("tidal://"), "https://desktop.tidal.com/");
}

#[test]
fn the_content_routes_still_open() {
    for (raw, expected) in [
        (
            "tidal://album/175122451",
            "https://desktop.tidal.com/album/175122451",
        ),
        ("tidal://track/9", "https://desktop.tidal.com/track/9"),
        ("tidal://artist/7", "https://desktop.tidal.com/artist/7"),
        (
            "tidal://playlist/abc-def",
            "https://desktop.tidal.com/playlist/abc-def",
        ),
        ("tidal://mix/m1", "https://desktop.tidal.com/mix/m1"),
        ("tidal://video/v1", "https://desktop.tidal.com/video/v1"),
        ("tidal://search?q=x", "https://desktop.tidal.com/search?q=x"),
        (
            "tidal://my-collection/albums",
            "https://desktop.tidal.com/my-collection/albums",
        ),
    ] {
        assert_eq!(target(raw), expected);
    }
}

#[test]
fn a_remainder_carrying_its_own_authority_is_refused() {
    // The graft alone would not save us here. These are the shapes that reparse
    // to somewhere other than our origin.
    for raw in [
        "tidal://evil.example.com/x",
        "tidal:///evil.example.com/x",
        "tidal://@evil.example.com/x",
    ] {
        match target_for(raw) {
            None => {}
            Some(target) => {
                let (absolute, route) = (target.to_string(), target.route().to_owned());
                assert!(
                    absolute.starts_with("https://desktop.tidal.com/"),
                    "{raw} escaped to {absolute}"
                );
                assert!(!route.starts_with("//"), "{raw} delivered {route}");
            }
        }
    }
}

#[test]
fn a_foreign_scheme_is_not_ours_to_route() {
    assert!(target_for("https://desktop.tidal.com/album/1").is_none());
    assert!(target_for("file:///etc/passwd").is_none());
    assert!(target_for("javascript:alert(1)").is_none());
    assert!(target_for("tidalx://album/1").is_none());
    assert!(target_for("tida://album/1").is_none());
    assert!(target_for("").is_none());
}

#[test]
fn every_route_that_redeems_a_credential_is_refused() {
    // A real callback arrives as a navigation inside our own browser; one handed
    // over by the OS carries someone else's code. `/login` was never the only
    // one. These three redeem a credential, and the fourth mutates state from
    // its query string with no credential at all.
    assert!(target_for("tidal://login/auth?code=stolen&state=x").is_none());
    assert!(target_for("tidal://login").is_none());
    assert!(target_for("tidal://oauth/spotify/return?claimId=1&code=stolen").is_none());
    assert!(target_for("tidal://lastfm/auth/stolen-token").is_none());
    assert!(target_for("tidal://marketplace/checkout/return?productType=ALBUM&itemId=1").is_none());
}

#[test]
fn a_spelling_the_tidal_router_would_still_match_is_refused() {
    // Its routes all compile case-insensitively and it decodes each segment
    // first; both of these reach the login page there. Comparing raw text
    // here would let them past.
    assert!(target_for("tidal://LOGIN/auth?code=stolen").is_none());
    assert!(target_for("tidal://%6cogin/auth?code=stolen").is_none());
    assert!(target_for("tidal://LoGiN").is_none());
}

#[test]
fn a_leading_empty_segment_does_not_pass_for_the_home_page() {
    // `tidal:///login/auth` grafts to a path of `//login/auth`, whose first
    // segment is empty. Reading that as "the home page" would hand the whole
    // allowlist a way round itself.
    assert!(target_for("tidal:///login/auth?code=stolen").is_none());
    assert!(target_for("tidal:////login/auth").is_none());
}

#[test]
fn a_route_nobody_has_read_is_refused_until_someone_has() {
    // Both take a whole query string into code this project has not read.
    assert!(target_for("tidal://view/anything").is_none());
    assert!(target_for("tidal://upload/share/save/track/1").is_none());
}

#[test]
fn an_unknown_route_is_refused_rather_than_guessed_at() {
    assert!(target_for("tidal://loginauth").is_none());
    assert!(target_for("tidal://whatever-tidal-adds-next").is_none());
}

#[test]
fn a_character_straddling_the_scheme_length_is_refused_not_fatal() {
    // Each of these puts a multi-byte character across the offset a byte-count
    // split would cut at. They arrive from a launch argument and from the two
    // listeners, making a panic here any local process's to trigger.
    for raw in ["tida\u{20AC}xy", "tida\u{1F600}", "tid\u{20AC}al://x"] {
        assert!(target_for(raw).is_none(), "{raw} was accepted");
    }
}

#[test]
fn a_hostile_argument_does_not_hide_the_link_behind_it() {
    let args = ["tida\u{20AC}xy", "tidal://album/1"].map(OsString::from);
    assert_eq!(url_from_args(args), Some("tidal://album/1".to_owned()));
}

#[test]
fn picks_the_scheme_argument_out_of_a_command_line() {
    let args = ["--enable-logging", "tidal://album/1", "--other"].map(OsString::from);
    assert_eq!(url_from_args(args), Some("tidal://album/1".to_owned()));
}

#[test]
fn a_command_line_without_our_scheme_yields_nothing() {
    let args = ["--type=renderer", "https://example.com"].map(OsString::from);
    assert_eq!(url_from_args(args), None);
    assert_eq!(url_from_args(Vec::new()), None);
}

/// A filename the platform accepts and `String` cannot hold, a legacy encoding
/// on unix or an unpaired surrogate on Windows. Both are what `%u` and the shell
/// really hand over, and reading argv as `String` aborts the launch on either.
fn undecodable_argument() -> OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![
            b'/', b'B', 0xF6, b'r', b'k', b'.', b'f', b'l', b'a', b'c',
        ])
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[0x002F, 0x0042, 0xD800, 0x0072, 0x006B])
    }
}

#[cfg(any(unix, windows))]
#[test]
fn an_argument_that_is_not_unicode_is_skipped_not_fatal() {
    let args = vec![undecodable_argument(), OsString::from("tidal://album/1")];
    assert_eq!(url_from_args(args), Some("tidal://album/1".to_owned()));
}

#[test]
fn surrounding_whitespace_does_not_defeat_the_scheme_check() {
    assert_eq!(
        target("  tidal://mix/abc\n"),
        "https://desktop.tidal.com/mix/abc"
    );
}

#[test]
fn a_doubled_slash_before_a_content_route_is_refused() {
    // The allowlist reads the first non-empty segment; `album` answered for
    // all three of these. What travels is the whole path, and a browser
    // resolving `//album/1` against our origin reads `album` as the HOST.
    assert!(target_for("tidal:///album/1").is_none());
    assert!(target_for("tidal:////album/1").is_none());
    assert!(target_for("tidal://///album/1").is_none());
}

#[test]
fn a_backslash_spells_the_same_escape_and_is_refused_too() {
    // A special scheme takes a backslash for a path separator, parsing both of
    // these to the doubled slash above. Naming the `//` spelling alone would
    // leave the other one open, which is how the same fix failed elsewhere.
    assert!(target_for("tidal:///\\album/1").is_none());
    assert!(target_for("tidal://\\/album/1").is_none());
}

#[test]
fn a_traversal_that_lands_on_a_doubled_slash_is_refused() {
    // Whether `..` neutralises the extra slash or leaves it depends on where it
    // sits; the shape of the result is what decides, not the presence of one.
    assert!(target_for("tidal://x/..//album/1").is_none());
    assert_eq!(route("tidal:///../album/1"), "/album/1");
}

#[test]
fn a_path_of_nothing_but_slashes_is_not_the_home_page() {
    // Only the bare scheme opens the home page. These deliver a path a browser
    // refuses outright, which reaches the renderer as a thrown exception.
    assert!(target_for("tidal:///").is_none());
    assert!(target_for("tidal://////////").is_none());
}

#[test]
fn the_route_delivered_is_the_spelling_that_was_matched() {
    // Both are accepted because the allowlist decodes and case-folds first.
    // Delivering the raw spelling instead points the launch at a URL TIDAL's
    // own host answers with a 403, and the app never boots.
    assert_eq!(route("tidal://Album/1"), "/album/1");
    assert_eq!(route("tidal://%61lbum/1"), "/album/1");
}

#[test]
fn only_the_segment_that_was_matched_is_rewritten() {
    // Nothing past the first segment was validated, leaving nothing there
    // normalised. Ids and share codes are case-sensitive, and re-encoding an
    // escape would change what it means.
    assert_eq!(route("tidal://album/AbC%20dEf"), "/album/AbC%20dEf");
    assert_eq!(
        route("tidal://playlist/A-b_C?q=Z%2Fz"),
        "/playlist/A-b_C?q=Z%2Fz"
    );
}

#[test]
fn every_delivered_route_starts_with_exactly_one_slash() {
    // The promise `route` makes to its two callers, that no accepted input hands
    // the renderer a string a browser would read as an authority.
    for raw in [
        "tidal://",
        "tidal://album/1",
        "tidal://album/",
        "tidal://Album/1",
        "tidal://search?q=x",
        "tidal://home",
    ] {
        let delivered = route(raw);
        assert!(delivered.starts_with('/'), "{raw} delivered {delivered}");
        assert!(!delivered.starts_with("//"), "{raw} delivered {delivered}");
    }
}
