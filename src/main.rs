#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod assistant;
mod catalog;
mod control;
mod device;
mod elgato;
mod engine;
mod gesture;
mod icons;
#[allow(dead_code)]
mod mcp_protocol;
mod mcp_server;
mod model;
mod recorder;
mod store;
mod theme;
mod ui;

#[derive(Debug)]
pub enum Event {
    Log(String),
    Key(usize, std::time::Instant),
    Device(bool, String),
    Applied(u64),
    Profile(String),
    Navigate(model::Step),
}

fn main() -> eframe::Result<()> {
    if std::env::args().any(|a| a == "--diagnostic") {
        device::diagnostic();
        return Ok(());
    }
    // One device owner per Windows session, including copies in other folders.
    let name = "Local\\DeckForgeD6\0".encode_utf16().collect::<Vec<_>>();
    let handle = unsafe {
        windows_sys::Win32::System::Threading::CreateMutexW(std::ptr::null(), 0, name.as_ptr())
    };
    if handle.is_null() {
        return Ok(());
    }
    struct Instance(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for Instance {
        fn drop(&mut self) {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.0);
            }
        }
    }
    let _instance = Instance(handle);
    if unsafe { windows_sys::Win32::Foundation::GetLastError() }
        == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS
    {
        let message = "DeckForge est déjà ouvert dans cette session.\0"
            .encode_utf16()
            .collect::<Vec<_>>();
        let title = "DeckForge\0".encode_utf16().collect::<Vec<_>>();
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                0,
            );
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--hardware-test") {
        device::hardware_test();
        return Ok(());
    }
    let size = if std::env::args().any(|a| a == "--small") {
        [1060.0, 720.0]
    } else {
        [1380.0, 900.0]
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("DeckForge · FIFINE D6")
            .with_inner_size(size)
            .with_min_inner_size([1060.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DeckForge",
        options,
        Box::new(|cc| Ok(Box::new(ui::App::new(cc)))),
    )
}
