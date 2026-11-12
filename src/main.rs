#![windows_subsystem = "windows"]

mod config;
mod gain;
mod logging;
mod tray;
mod voicemeeter;
mod volume_state_type;
mod windows_eco_mode;
mod windows_volume;

use std::{sync::OnceLock, time::Duration};

use anyhow::Context;
use smol::{
    channel::{self, Receiver, Sender},
    future::FutureExt,
};
use volume_state_type::VolumeState;
use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole, SetConsoleCtrlHandler};
use windows_core::BOOL;

pub const PACKAGE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"));

/// Lowercase part of the name of the Windows playback device that feeds Voicemeeter's first virtual input.
const DEVICE_NAME: &str = "voicemeeter input";

/// How often to check that the device and Voicemeeter are still there, in case a notification was missed.
const RECHECK_INTERVAL: Duration = Duration::from_secs(2);

/// Device changes come in bursts (removed, added, state changed); let them settle before looking again.
const DEVICE_SETTLE_DELAY: Duration = Duration::from_millis(300);

pub enum AppEvent {
    Exit,
    SetMaxGain(f32),
}

pub fn show_error(err: &anyhow::Error) {
    log!("Error: {err:?}");

    let _ = win_msgbox::error::<win_msgbox::Okay>(&format!("{err:?}"))
        .title(concat!(env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"), " Error"))
        .show();
}

fn main() {
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };

    logging::init();
    log!("{PACKAGE_NAME} started, log file: {}", logging::log_path().display());

    if let Err(err) = smol::block_on(run()) {
        show_error(&err);
    }

    log!("Exiting");
}

async fn run() -> anyhow::Result<()> {
    windows_volume::initialize_com().context("COM initialization failed")?;

    if let Err(err) = windows_eco_mode::set_eco_mode_for_current_process() {
        log!("info: Failed to set Process mode to Eco: {err}");
    }

    // Only fails when the Remote API itself is unavailable, i.e. Voicemeeter isn't installed.
    // Voicemeeter not running (yet) is fine and handled by the sync loop.
    let mut link = voicemeeter::Link::new()
        .context("Couldn't load the Voicemeeter Remote API. Is Voicemeeter installed?")?;

    let (events_tx, events_rx) = channel::unbounded();
    install_ctrl_handler(events_tx.clone());

    let max_gain_db = config::load_max_gain();
    let tray = tray::spawn(events_tx, max_gain_db)?;

    let result = sync_loop(&mut link, events_rx, max_gain_db).await;

    tray.close();
    result
}

/// Keeps Voicemeeter in sync with the Windows volume until asked to exit.
///
/// Nothing in here is fatal: a missing device or a missing/restarting Voicemeeter
/// just means waiting until they come back.
async fn sync_loop(
    link: &mut voicemeeter::Link,
    events: Receiver<AppEvent>,
    mut max_gain_db: f32,
) -> anyhow::Result<()> {
    let (volume_tx, volume_rx) = channel::bounded::<()>(1);
    let (devices_tx, devices_rx) = channel::bounded::<()>(1);

    let devices = windows_volume::DeviceWatcher::new(devices_tx)?;

    let mut observer: Option<windows_volume::VolumeObserver> = None;
    let mut rescan = true;
    let mut waiting_for_device_logged = false;
    let mut vm_connected: Option<bool> = None;
    let mut last_synced: Option<(VolumeState, f32)> = None;
    let mut last_error = String::new();

    log!("Max volume: {}", config::max_gain_label(max_gain_db));

    loop {
        if rescan || observer.is_none() {
            rescan = false;
            // Drop (and unregister) the old observer before creating a new one
            observer = None;

            match devices.observe(DEVICE_NAME, volume_tx.clone()) {
                Ok(Some(found)) => {
                    log!("Listening for volume changes of \"{}\"", found.name());
                    observer = Some(found);
                    waiting_for_device_logged = false;
                }
                Ok(None) if !waiting_for_device_logged => {
                    log!("Waiting for the \"VoiceMeeter Input\" playback device to become active...");
                    waiting_for_device_logged = true;
                }
                Ok(None) => {}
                Err(err) => log_once(&mut last_error, format!("{err:#}")),
            }
        }

        if let Some(current) = &observer {
            match current.state() {
                Err(err) => {
                    log!("Lost the audio device ({err}), looking for it again");
                    observer = None;
                }
                Ok(state) => {
                    let connected = link.is_connected();
                    if vm_connected != Some(connected) {
                        vm_connected = Some(connected);
                        if connected {
                            log!("Connected to {}", link.program_name());
                            // Voicemeeter (re)started: push the current volume again
                            last_synced = None;
                        } else {
                            log!("Voicemeeter is not running, waiting for it...");
                        }
                    }

                    let gain_db = gain::volume_to_gain(state.volume, max_gain_db);

                    if connected && last_synced != Some((state, gain_db)) {
                        match link.apply(state, gain_db) {
                            Ok(()) => {
                                log!(
                                    "{:.0}%{} -> {gain_db:+.1} dB",
                                    state.volume * 100.0,
                                    if state.is_muted { " (muted)" } else { "" }
                                );
                                last_synced = Some((state, gain_db));
                                last_error.clear();
                            }
                            Err(err) => log_once(
                                &mut last_error,
                                format!("Couldn't update Voicemeeter, will retry: {err:#}"),
                            ),
                        }
                    }
                }
            }
        }

        enum Wake {
            Volume,
            Devices,
            Event(AppEvent),
            Tick,
        }

        let wake = async {
            let _ = volume_rx.recv().await;
            Wake::Volume
        }
        .or(async {
            let _ = devices_rx.recv().await;
            Wake::Devices
        })
        .or(async { Wake::Event(events.recv().await.unwrap_or(AppEvent::Exit)) })
        .or(async {
            smol::Timer::after(RECHECK_INTERVAL).await;
            Wake::Tick
        })
        .await;

        match wake {
            Wake::Volume | Wake::Tick => {}
            Wake::Devices => {
                smol::Timer::after(DEVICE_SETTLE_DELAY).await;
                let _ = devices_rx.try_recv();
                rescan = true;
            }
            Wake::Event(AppEvent::Exit) => return Ok(()),
            Wake::Event(AppEvent::SetMaxGain(db)) => {
                max_gain_db = db;
                log!("Max volume: {}", config::max_gain_label(db));
                if let Err(err) = config::save_max_gain(db) {
                    log!("Couldn't save the max volume setting: {err}");
                }
            }
        }
    }
}

/// Log `message` unless it is the same as the previous one, to avoid flooding the log while retrying.
fn log_once(last: &mut String, message: String) {
    if *last != message {
        log!("{message}");
        *last = message;
    }
}

/// Exit cleanly (logging out of Voicemeeter, removing the tray icon) on Ctrl+C when run from a terminal.
fn install_ctrl_handler(events: Sender<AppEvent>) {
    static EVENTS: OnceLock<Sender<AppEvent>> = OnceLock::new();

    unsafe extern "system" fn handler(_ctrl_type: u32) -> BOOL {
        if let Some(events) = EVENTS.get() {
            let _ = events.try_send(AppEvent::Exit);
        }
        BOOL(1)
    }

    let _ = EVENTS.set(events);
    let _ = unsafe { SetConsoleCtrlHandler(Some(handler), true) };
}
