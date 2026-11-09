pub fn copy_to_clipboard(text: &str) {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        if let Ok(mut child) = Command::new("/usr/bin/pbcopy")
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }
    #[cfg(windows)]
    if let Err(e) = windows_clipboard::copy(text) {
        tracing::warn!(%e, "não foi possível copiar para a área de transferência");
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = text;
}

#[cfg(windows)]
mod windows_clipboard {
    use windows::Win32::Foundation::{GlobalFree, HANDLE};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    const CF_UNICODETEXT: u32 = 13;

    pub fn copy(text: &str) -> windows::core::Result<()> {
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        unsafe {
            OpenClipboard(None)?;
            let result = fill(&wide);
            let _ = CloseClipboard();
            result
        }
    }

    unsafe fn fill(wide: &[u16]) -> windows::core::Result<()> {
        EmptyClipboard()?;
        let memory = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of_val(wide))?;
        let target = GlobalLock(memory).cast::<u16>();
        if target.is_null() {
            let _ = GlobalFree(Some(memory));
            return Err(windows::core::Error::from_thread());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
        let _ = GlobalUnlock(memory);
        if let Err(e) = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.0))) {
            let _ = GlobalFree(Some(memory));
            return Err(e);
        }
        Ok(())
    }
}
