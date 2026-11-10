use std::sync::atomic::{AtomicI32, Ordering};

use objc2_app_kit::{
    NSApplicationActivationOptions, NSRunningApplication, NSWindow, NSWindowCollectionBehavior,
    NSWorkspace,
};
use tauri::{Runtime, WebviewWindow};

pub fn join_all_spaces_and_fullscreen<R: Runtime>(window: &WebviewWindow<R>) {
    let Ok(ptr) = window.ns_window() else {
        return;
    };
    if ptr.is_null() {
        return;
    }
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    let behavior = ns_window.collectionBehavior()
        | NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::Stationary
        | NSWindowCollectionBehavior::IgnoresCycle;
    ns_window.setCollectionBehavior(behavior);
}

static PREVIOUS_APP: AtomicI32 = AtomicI32::new(0);

pub fn remember_frontmost_app() {
    let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
        return;
    };
    let pid = app.processIdentifier();
    if pid > 0 && pid as u32 != std::process::id() {
        PREVIOUS_APP.store(pid, Ordering::Relaxed);
    }
}

pub fn restore_frontmost_app() {
    if !NSRunningApplication::currentApplication().isActive() {
        return;
    }
    let pid = PREVIOUS_APP.load(Ordering::Relaxed);
    if pid <= 0 {
        return;
    }
    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        app.activateWithOptions(NSApplicationActivationOptions::empty());
    }
}

pub fn announce(text: &str) {
    use objc2::runtime::AnyObject;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAccessibilityAnnouncementKey, NSAccessibilityAnnouncementRequestedNotification,
        NSAccessibilityPostNotificationWithUserInfo, NSAccessibilityPriorityKey,
        NSAccessibilityPriorityLevel, NSApplication,
    };
    use objc2_foundation::{NSDictionary, NSNumber, NSString};

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let message = NSString::from_str(text);
    let priority = NSNumber::new_isize(NSAccessibilityPriorityLevel::High.0);
    let keys: [&NSString; 2] =
        unsafe { [NSAccessibilityAnnouncementKey, NSAccessibilityPriorityKey] };
    let values: [&AnyObject; 2] = [message.as_ref(), priority.as_ref()];
    let info = NSDictionary::from_slices(&keys, &values);
    unsafe {
        NSAccessibilityPostNotificationWithUserInfo(
            app.as_ref(),
            NSAccessibilityAnnouncementRequestedNotification,
            Some(&info),
        );
    }
}
