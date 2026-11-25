use anyhow::{Context as _, Result, ensure};
use windows::Win32::{
    Foundation::HWND,
    System::Threading::{AttachThreadInput, GetCurrentThreadId},
    UI::{
        Input::KeyboardAndMouse::SetActiveWindow,
        WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsWindow,
            SetForegroundWindow,
        },
    },
};

pub(crate) fn acquire_foreground_window(hwnd: HWND) -> Result<()> {
    ensure!(
        hwnd != HWND::default() && unsafe { IsWindow(Some(hwnd)).as_bool() },
        "native foreground preparation requires a live HWND"
    );

    let current_thread = unsafe { GetCurrentThreadId() };
    let foreground_thread = unsafe {
        let foreground = GetForegroundWindow();
        (foreground != HWND::default()).then(|| GetWindowThreadProcessId(foreground, None))
    }
    .unwrap_or_default();
    let target_thread = unsafe { GetWindowThreadProcessId(hwnd, None) };
    ensure!(
        target_thread != 0,
        "native foreground preparation could not resolve the HWND thread"
    );

    let mut input_attachment = NativeTestInputAttachment::new(current_thread);
    input_attachment.attach(foreground_thread);
    input_attachment.attach(target_thread);

    unsafe {
        BringWindowToTop(hwnd).context("failed to raise the native test foreground HWND")?;
        let _ = SetActiveWindow(hwnd);
        ensure!(
            SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() == hwnd,
            "Windows rejected the native test foreground HWND"
        );
    }
    Ok(())
}

struct NativeTestInputAttachment {
    current_thread: u32,
    attached_threads: Vec<u32>,
}

impl NativeTestInputAttachment {
    fn new(current_thread: u32) -> Self {
        Self {
            current_thread,
            attached_threads: Vec::new(),
        }
    }

    fn attach(&mut self, thread: u32) {
        if thread == 0 || thread == self.current_thread || self.attached_threads.contains(&thread) {
            return;
        }
        if unsafe { AttachThreadInput(self.current_thread, thread, true).as_bool() } {
            self.attached_threads.push(thread);
        }
    }
}

impl Drop for NativeTestInputAttachment {
    fn drop(&mut self) {
        for thread in self.attached_threads.drain(..).rev() {
            let _ = unsafe { AttachThreadInput(self.current_thread, thread, false) };
        }
    }
}
