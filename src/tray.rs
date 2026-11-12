//! Tray icon running its own Win32 message loop on a dedicated thread.

use crate::{AppEvent, config, log, logging};

use anyhow::Context;
use smol::channel::Sender;
use std::os::windows::io::AsRawHandle;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
};
use windows::Win32::{
    Foundation::{HANDLE, LPARAM, WPARAM},
    System::Threading::GetThreadId,
    UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, MSG, PostThreadMessageW, TranslateMessage, WM_QUIT},
};

/// Right after logon the taskbar may not exist yet, so creating the icon is retried for a while.
const CREATE_ATTEMPTS: u32 = 60;
const CREATE_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

pub struct Tray {
    thread: std::thread::JoinHandle<()>,
}

pub fn spawn(events: Sender<AppEvent>, max_gain_db: f32) -> anyhow::Result<Tray> {
    let thread = std::thread::Builder::new()
        .name("Tray icon".into())
        .spawn(move || {
            if let Err(err) = run(&events, max_gain_db) {
                crate::show_error(&err);
            }
            let _ = events.try_send(AppEvent::Exit);
        })
        .context("Couldn't start the tray icon thread")?;

    Ok(Tray { thread })
}

impl Tray {
    /// Stop the message loop and wait for the icon to be removed.
    pub fn close(self) {
        if !self.thread.is_finished() {
            let id = unsafe { GetThreadId(HANDLE(self.thread.as_raw_handle())) };
            // Fails if the loop isn't running yet (still creating the icon); just leave then.
            if id == 0 || unsafe { PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0)) }.is_err() {
                return;
            }
        }
        let _ = self.thread.join();
    }
}

fn run(events: &Sender<AppEvent>, mut max_gain_db: f32) -> anyhow::Result<()> {
    let max_gain_items: Vec<(CheckMenuItem, f32)> = config::MAX_GAIN_CHOICES
        .iter()
        .map(|&db| {
            let item = CheckMenuItem::new(config::max_gain_label(db), true, db == max_gain_db, None);
            (item, db)
        })
        .collect();

    let max_gain_menu = Submenu::new("Volume at 100%", true);
    for (item, _) in &max_gain_items {
        max_gain_menu.append(item)?;
    }

    let open_log = MenuItem::new("Open log", true, None);
    let exit = MenuItem::new("Exit", true, None);

    let menu = Menu::new();
    menu.append_items(&[
        &max_gain_menu,
        &open_log,
        &PredefinedMenuItem::separator(),
        &exit,
    ])?;

    let _tray_icon = create_icon(&menu)?;

    let mut msg = MSG::default();
    // GetMessageW returns 0 on WM_QUIT and -1 on error
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == *exit.id() {
                return Ok(());
            } else if event.id == *open_log.id() {
                let _ = std::process::Command::new("notepad.exe")
                    .arg(logging::log_path())
                    .spawn();
            } else if let Some(&(_, db)) = max_gain_items.iter().find(|(item, _)| *item.id() == event.id) {
                // Check items toggle themselves on click, keep exactly one checked
                for (item, item_db) in &max_gain_items {
                    item.set_checked(*item_db == db);
                }
                if db != max_gain_db {
                    max_gain_db = db;
                    let _ = events.try_send(AppEvent::SetMaxGain(db));
                }
            }
        }
    }

    Ok(())
}

fn create_icon(menu: &Menu) -> anyhow::Result<TrayIcon> {
    let mut attempt = 1;
    loop {
        let result = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_tooltip(crate::PACKAGE_NAME)
            .with_icon(Icon::from_resource(1, None).context("Failed to read icon resource")?)
            .build();

        match result {
            Ok(icon) => return Ok(icon),
            Err(err) if attempt < CREATE_ATTEMPTS => {
                if attempt == 1 {
                    log!("Couldn't create the tray icon yet ({err}), retrying...");
                }
                attempt += 1;
                std::thread::sleep(CREATE_RETRY_DELAY);
            }
            Err(err) => return Err(err).context("Failed to build tray icon"),
        }
    }
}
