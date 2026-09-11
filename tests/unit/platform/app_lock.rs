//! Tests for the Linux half of `src/platform/app_lock.rs`, attached by `#[path]`.

use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixDatagram};

use super::HandOff;
use super::linux_impl::*;

/// A receiver on an abstract address unique to this test.
fn listening(tag: &str) -> (SocketAddr, UnixDatagram) {
    let addr =
        SocketAddr::from_abstract_name(format!("tidalunar-test-{tag}-{}", std::process::id()))
            .expect("an abstract address");
    let socket = UnixDatagram::bind_addr(&addr).expect("bind the receiver");
    (addr, socket)
}

#[test]
fn a_datagram_from_our_own_user_is_ours_to_act_on() {
    assert_eq!(classify(Some(1002), 1002), Sender::Ours);
}

#[test]
fn a_datagram_from_another_user_is_not_ours() {
    // The reason this rule exists: an abstract socket has no permission model at
    // all, and this address is a hash of a path that is not a secret; any
    // process owned by any other local user can reach it.
    assert_eq!(classify(Some(0), 1002), Sender::Foreign(0));
    assert_eq!(classify(Some(1003), 1002), Sender::Foreign(1003));
}

#[test]
fn a_datagram_the_kernel_did_not_vouch_for_is_not_ours() {
    // Not the same fact as a stranger, and just as unusable, since with no
    // credential there is nobody to attribute the datagram to. This is also what a failure
    // to enable credential passing looks like, which is why it refuses.
    assert_eq!(classify(None, 1002), Sender::Unattributed);
}

#[test]
fn a_link_that_fits_reaches_the_running_instance() {
    let (addr, socket) = listening("fits");

    assert_eq!(hand_over(&addr, Some("tidal://album/1")), HandOff::Landed);

    let mut buf = [0u8; MAX_SIGNAL];
    let n = socket.recv(&mut buf).expect("receive");
    assert_eq!(&buf[..n], b"tidal://album/1");
}

#[test]
fn an_over_long_link_still_brings_the_window_forward() {
    // One socket carries both meanings here; losing the link must not take the
    // focus signal with it. Refused before the send rather than truncated on
    // arrival (a cut URL reads as a refused route at the far end).
    let (addr, socket) = listening("toolong");
    let long = format!("tidal://album/{}", "9".repeat(MAX_SIGNAL));

    assert_eq!(hand_over(&addr, Some(&long)), HandOff::TooLong);

    let mut buf = [0u8; MAX_SIGNAL];
    let n = socket.recv(&mut buf).expect("receive");
    assert_eq!(
        &buf[..n],
        FOCUS_SIGNAL.as_bytes(),
        "the window must still come forward when the link cannot be sent"
    );
}

#[test]
fn a_launch_carrying_nothing_asks_only_for_focus() {
    let (addr, socket) = listening("focus");

    assert_eq!(hand_over(&addr, None), HandOff::Landed);

    let mut buf = [0u8; MAX_SIGNAL];
    let n = socket.recv(&mut buf).expect("receive");
    assert_eq!(&buf[..n], FOCUS_SIGNAL.as_bytes());
}

#[test]
fn an_address_nobody_holds_is_named_rather_than_swallowed() {
    // The abstract name auto-reclaims when the owner dies, making this the real
    // shape of a survivor that exited between our failed bind and this send.
    let addr =
        SocketAddr::from_abstract_name(format!("tidalunar-test-gone-{}", std::process::id()))
            .expect("an abstract address");

    assert_eq!(
        hand_over(&addr, Some("tidal://album/1")),
        HandOff::NoListener
    );
}

#[test]
fn the_kernel_reports_the_real_sender_of_a_datagram() {
    // The mechanism against the real kernel rather than the rule above. Without
    // credential passing enabled on the receiver, no control message arrives and
    // there is nothing to check.
    //
    // A child forked here would carry this same uid, leaving a genuinely foreign
    // one impossible to produce on one account. What it would do is settled by
    // `a_datagram_from_another_user_is_not_ours`, not here.
    let addr =
        SocketAddr::from_abstract_name(format!("tidalunar-test-creds-{}", std::process::id()))
            .expect("an abstract address");
    let socket = UnixDatagram::bind_addr(&addr).expect("bind the receiver");
    enable_peer_credentials(&socket).expect("enable credential passing");

    let client = UnixDatagram::unbound().expect("an unbound sender");
    client.connect_addr(&addr).expect("connect to the receiver");
    client.send(FOCUS_SIGNAL.as_bytes()).expect("send");

    let mut buf = [0u8; MAX_SIGNAL];
    let (n, peer) = recv_with_sender(&socket, &mut buf).expect("receive");

    assert_eq!(&buf[..n], FOCUS_SIGNAL.as_bytes());
    assert_eq!(
        peer,
        Some(own_uid()),
        "the kernel did not attribute a datagram we sent ourselves"
    );
}
