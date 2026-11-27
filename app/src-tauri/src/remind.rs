//! Once a day a notification "N new photos to review"; clicking it opens Review.
//! Only with a team gallery set up (otherwise there is nothing to review), not
//! while paused, not while the window is open, not before REMIND_FROM o'clock.
//! Off with `git config --global lolcommits.remind false`.
use crate::{config, share, store, time};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;
use tauri::{AppHandle, Manager};

const REMIND_FROM: u32 = 10;
static ON_CLICK: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();
/// A reminder is waiting for macOS (e.g. the permission question is open).
static WAITING: AtomicBool = AtomicBool::new(false);

pub fn enabled() -> bool {
    config::get_or("remind", "true") != "false"
}

/// Day of the last reminder, so there is at most one per day.
fn last_path() -> std::path::PathBuf {
    config::lol_dir().join("reminded")
}

/// Call once at startup, on the main thread. `on_click` runs when the
/// notification is clicked, also when that starts the app.
pub fn start(app: AppHandle, on_click: impl Fn() + Send + Sync + 'static) {
    let _ = ON_CLICK.set(Box::new(on_click));
    platform::init();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(300));
        let window_open = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
        if WAITING.load(Ordering::SeqCst) {
            continue;
        }
        if let Some(n) = due(window_open) {
            WAITING.store(true, Ordering::SeqCst);
            // done for today once macOS has it, or once notifications are turned off
            platform::notify(&if n == 1 { "1 new photo to review".to_string() } else { format!("{n} new photos to review") }, || {
                let _ = std::fs::write(last_path(), time::today().to_string());
                WAITING.store(false, Ordering::SeqCst);
            });
        }
    });
}

/// How many photos to remind of, if a reminder is due now.
fn due(window_open: bool) -> Option<usize> {
    if !enabled() || window_open || !share::configured() || config::paused_until().is_some() || time::hour() < REMIND_FROM {
        return None;
    }
    let last = std::fs::read_to_string(last_path()).unwrap_or_default();
    if last.trim() == time::today().to_string() {
        return None;
    }
    Some(store::to_review(&store::shots())).filter(|n| *n > 0)
}

#[cfg(target_os = "macos")]
mod platform {
    use super::ON_CLICK;
    use crate::store;
    use block2::{DynBlock, RcBlock};
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, AllocAnyThread};
    use objc2_foundation::{NSBundle, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationPresentationOptions,
        UNNotificationRequest, UNNotificationResponse, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "LolcommitsNotificationDelegate"]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl UNUserNotificationCenterDelegate for Delegate {
            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(&self, _center: &UNUserNotificationCenter, _response: &UNNotificationResponse, done: &DynBlock<dyn Fn()>) {
                if let Some(f) = ON_CLICK.get() {
                    f();
                }
                done.call(());
            }

            // also show it while lolcommits is the active app
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(&self, _center: &UNUserNotificationCenter, _n: &UNNotification, done: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>) {
                done.call((UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::List,));
            }
        }
    );

    /// Notifications need a real app bundle; `tauri dev` runs a bare binary.
    fn bundled() -> bool {
        NSBundle::mainBundle().bundleIdentifier().is_some()
    }

    pub fn init() {
        if !bundled() {
            return;
        }
        let delegate: Retained<Delegate> = unsafe { msg_send![Delegate::alloc(), init] };
        UNUserNotificationCenter::currentNotificationCenter().setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        std::mem::forget(delegate); // the center holds it weakly; it lives as long as the app
    }

    pub fn notify(body: &str, done: impl Fn() + 'static) {
        if !bundled() {
            store::log(&format!("reminder (no app bundle, not shown): {body}"));
            done();
            return;
        }
        let body = body.to_string();
        // asks for permission the first time (the answer may take a while);
        // later calls return the person's choice at once
        let then = RcBlock::new(move |granted: Bool, _err: *mut NSError| {
            if !granted.as_bool() {
                done();
                return;
            }
            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str("lolcommits"));
            content.setBody(&NSString::from_str(&body));
            // same identifier: a new reminder replaces yesterday's in the Notification Center
            let req = UNNotificationRequest::requestWithIdentifier_content_trigger(&NSString::from_str("review"), &content, None);
            UNUserNotificationCenter::currentNotificationCenter().addNotificationRequest_withCompletionHandler(&req, None);
            done();
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound, &then);
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn init() {}
    pub fn notify(body: &str, done: impl Fn() + 'static) {
        crate::store::log(&format!("reminder (notifications are macOS only): {body}"));
        done();
    }
}
