//! Ícone da bandeja: o pato de perfil (gerado por tools/make_tray_icon.py).
//!
//! - macOS: imagem "template" (só a silhueta; o sistema pinta conforme o tema claro ou
//!   escuro), montada como um NSImage com as versões 1x, 2x e 3x, no botão da barra.
//! - Windows e Linux: versão colorida (não suportam template).
//! - Alerta (bolinha no ícone): quando aparece um aviso que pede atenção, como uma
//!   checagem de PR que falhou ou um "precisa de você" do Claude. Volta ao normal quando
//!   a pessoa abre o menu ou quando esses avisos somem da tela.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager};

use crate::model::{Notification, Source, Status};

pub const TRAY_ID: &str = "quacko-tray";

/// Estado de alerta do ícone.
#[derive(Default)]
pub struct TrayAlert(AtomicBool);

/// Aviso que liga o alerta: checagem de PR que falhou ou pedido de atenção.
pub fn is_alert(n: &Notification) -> bool {
    n.status == Status::Attention || (n.status == Status::Fail && n.source == Source::Ci)
}

/// Ícone usado na criação da bandeja (no macOS ele é trocado em seguida pelo NSImage
/// com as três resoluções).
pub fn initial_image() -> tauri::image::Image<'static> {
    #[cfg(target_os = "macos")]
    {
        tauri::include_image!("icons/tray/trayTemplate@2x.png")
    }
    #[cfg(not(target_os = "macos"))]
    {
        tauri::include_image!("icons/tray/tray-color@2x.png")
    }
}

/// Liga ou desliga o alerta (só redesenha quando muda).
pub fn set_alert(app: &AppHandle, on: bool) {
    let Some(state) = app.try_state::<TrayAlert>() else {
        return;
    };
    if state.0.swap(on, Ordering::SeqCst) == on {
        return;
    }
    tracing::debug!(on, "alerta do ícone da bandeja");
    apply(app, on);
}

/// Desenha o ícone do estado atual (usado logo depois de criar a bandeja).
pub fn apply_current(app: &AppHandle) {
    let on = app
        .try_state::<TrayAlert>()
        .is_some_and(|s| s.0.load(Ordering::SeqCst));
    apply(app, on);
}

#[cfg(target_os = "macos")]
fn apply(app: &AppHandle, alert: bool) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    // Tudo dentro do closure: ele roda na thread principal (o NSStatusItem não pode sair dela).
    let _ = tray.with_inner_tray_icon(move |inner| {
        let (Some(item), Some(mtm)) = (inner.ns_status_item(), objc2::MainThreadMarker::new())
        else {
            return;
        };
        if let (Some(button), Some(image)) = (item.button(mtm), macos::template_image(alert)) {
            button.setImage(Some(&image));
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn apply(app: &AppHandle, alert: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let image = if alert {
            tauri::include_image!("icons/tray/tray-color-alert@2x.png")
        } else {
            tauri::include_image!("icons/tray/tray-color@2x.png")
        };
        let _ = tray.set_icon(Some(image));
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_app_kit::{NSBitmapImageRep, NSImage};
    use objc2_foundation::{NSData, NSSize};

    /// Tamanho do desenho em pontos; as versões 2x e 3x dão nitidez em telas Retina.
    const POINTS: f64 = 16.0;

    const NORMAL: [&[u8]; 3] = [
        include_bytes!("../icons/tray/trayTemplate.png"),
        include_bytes!("../icons/tray/trayTemplate@2x.png"),
        include_bytes!("../icons/tray/trayTemplate@3x.png"),
    ];
    const ALERT: [&[u8]; 3] = [
        include_bytes!("../icons/tray/trayAlertTemplate.png"),
        include_bytes!("../icons/tray/trayAlertTemplate@2x.png"),
        include_bytes!("../icons/tray/trayAlertTemplate@3x.png"),
    ];

    /// NSImage template com as três resoluções: o macOS escolhe a certa para a tela e
    /// pinta a silhueta com a cor do tema.
    pub fn template_image(alert: bool) -> Option<Retained<NSImage>> {
        let size = NSSize::new(POINTS, POINTS);
        let image = NSImage::initWithSize(NSImage::alloc(), size);
        for bytes in if alert { ALERT } else { NORMAL } {
            let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(bytes))?;
            rep.setSize(size);
            image.addRepresentation(&rep);
        }
        image.setTemplate(true);
        Some(image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(source: Source, status: Status) -> Notification {
        Notification {
            id: 1,
            source,
            status,
            message: None,
            title: None,
            project: None,
            url: None,
            ttl_ms: 8000,
            progress: None,
            sticky: false,
        }
    }

    #[test]
    fn quais_avisos_ligam_o_alerta() {
        assert!(
            is_alert(&n(Source::Ci, Status::Fail)),
            "checagem de PR que falhou"
        );
        assert!(
            is_alert(&n(Source::Agent, Status::Attention)),
            "Claude precisa de você"
        );
        assert!(is_alert(&n(Source::Claude, Status::Attention)));
        assert!(!is_alert(&n(Source::Ci, Status::Ok)));
        assert!(
            !is_alert(&n(Source::Ci, Status::Info)),
            "progresso sem falha"
        );
        assert!(!is_alert(&n(Source::Agent, Status::Ok)));
        assert!(
            !is_alert(&n(Source::Custom, Status::Fail)),
            "falha de script qualquer não é PR"
        );
    }

    #[test]
    fn os_doze_icones_existem_nos_tamanhos_certos() {
        for (name, px) in [
            ("trayTemplate.png", 16),
            ("trayTemplate@2x.png", 32),
            ("trayTemplate@3x.png", 48),
            ("trayAlertTemplate@3x.png", 48),
            ("tray-color.png", 16),
            ("tray-color-alert@2x.png", 32),
        ] {
            let bytes =
                std::fs::read(format!("{}/icons/tray/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
            // Largura e altura ficam nos bytes 16..24 do cabeçalho IHDR do PNG.
            let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
            assert_eq!((w, h), (px, px), "{name}");
        }
    }
}
