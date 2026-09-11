//! macOS: receive a `tidal://` link from Launch Services.
//!
//! The other two platforms are handed the URL on a command line. macOS never is.
//! Launch Services activates the app that already runs and sends it a `'GURL'`
//! Apple Event, producing no second process and no argument to read. CEF exposes
//! no hook for it, leaving the registration to the embedder.
//!
//! The two methods that carry a Carbon keyword have typed bindings behind
//! `objc2-core-services`; a whole crate for two integers is not worth it, and
//! those two are sent by hand while the rest stays typed.
#![cfg(target_os = "macos")]

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AnyThread, define_class, msg_send, sel};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSString};

/// `'GURL'`, both the event class and the event id Launch Services sends.
const GET_URL: u32 = u32::from_be_bytes(*b"GURL");
/// `'----'`, the keyword holding an Apple Event's direct parameter.
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

define_class!(
    /// Exists only to own the selector the event manager calls back on.
    #[unsafe(super(NSObject))]
    struct UrlEventHandler;

    impl UrlEventHandler {
        #[unsafe(method(handleGetURLEvent:withReplyEvent:))]
        fn handle_get_url(
            &self,
            event: &NSAppleEventDescriptor,
            _reply: &NSAppleEventDescriptor,
        ) {
            // SAFETY: the keyword is a FourCharCode and the receiver is the
            // event descriptor the manager passed in.
            let direct: Option<Retained<NSAppleEventDescriptor>> =
                unsafe { msg_send![event, paramDescriptorForKeyword: DIRECT_OBJECT] };
            let Some(direct) = direct else {
                return;
            };
            let Some(url): Option<Retained<NSString>> = direct.stringValue() else {
                return;
            };
            crate::ui::deep_link::deliver(&url.to_string());
        }
    }
);

/// Claim `'GURL'` for this process. Must run before the app can be activated by
/// a link, and the event manager does not retain its handler, so the one built
/// here is deliberately leaked (it answers for the life of the process).
pub(crate) fn install() {
    let this = UrlEventHandler::alloc().set_ivars(());
    // SAFETY: `init` on a freshly allocated instance of our own class.
    let handler: Retained<UrlEventHandler> = unsafe { msg_send![super(this), init] };

    let manager = NSAppleEventManager::sharedAppleEventManager();
    let target: &AnyObject = &handler;
    // SAFETY: the selector matches the method defined above, and both codes are
    // the FourCharCodes the typed binding would pass.
    unsafe {
        let _: () = msg_send![
            &*manager,
            setEventHandler: target,
            andSelector: sel!(handleGetURLEvent:withReplyEvent:),
            forEventClass: GET_URL,
            andEventID: GET_URL,
        ];
    }
    std::mem::forget(handler);

    crate::vprintln!("[SCHEME] Apple Event handler installed");
}
