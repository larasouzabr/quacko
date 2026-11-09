//! Ajustes nativos do macOS que o Tauri não expõe.

use std::sync::atomic::{AtomicI32, Ordering};

use objc2_app_kit::{
    NSApplicationActivationOptions, NSRunningApplication, NSWindow, NSWindowCollectionBehavior,
    NSWorkspace,
};
use tauri::{Runtime, WebviewWindow};

/// Aparece em todos os Spaces e por cima de apps em tela cheia.
/// Precisa rodar na thread principal (o `setup` do Tauri roda nela).
pub fn join_all_spaces_and_fullscreen<R: Runtime>(window: &WebviewWindow<R>) {
    let Ok(ptr) = window.ns_window() else {
        return;
    };
    if ptr.is_null() {
        return;
    }
    // SAFETY: `ns_window()` devolve o NSWindow vivo desta janela; estamos na thread principal.
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    let behavior = ns_window.collectionBehavior()
        | NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::Stationary
        | NSWindowCollectionBehavior::IgnoresCycle;
    ns_window.setCollectionBehavior(behavior);
}

/// pid do último app em primeiro plano que não é o Miau.
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

/// Se um clique no cartão ativou o Miau, devolve o foco ao app anterior.
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
