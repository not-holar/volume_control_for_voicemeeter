#![windows_subsystem = "windows"]

mod voicemeeter;
mod volume_state_type;
mod windows_eco_mode;
mod windows_volume;

use std::sync::Arc;

use anyhow::Context;
use lerp::Lerp;
use smol::future::FutureExt;
use tokio::sync::Notify;

macro_rules! package_name {
    () => {
        concat!(env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"))
    };
}

fn print_error(err: &impl std::fmt::Debug) {
    fn format_error(err: &impl std::fmt::Debug) -> String {
        format!("{err:?}")
    }
    println!("{}", format_error(err));

    let _ = win_msgbox::error::<win_msgbox::Okay>(format_error(err).as_str())
        .title(concat!(package_name!(), " Error"))
        .show()
        .map_err(anyhow::Error::msg)
        .context("Failed to display win_msgbox error popup")
        .inspect_err(|err| println!("{}", format_error(err)));
}

fn main() {
    let _ = unsafe {
        use windows::Win32::System::Console::*;
        AttachConsole(ATTACH_PARENT_PROCESS)
    };

    println!("Started");

    let _ = smol::block_on(listen()).inspect_err(|err| {
        print_error(err);

        // println!("\nPress ENTER to continue...");
        // std::io::stdin().lines().next(););
    });

    println!("Exiting safely");
}

async fn listen() -> anyhow::Result<()> {
    // Initialize Win32's COM libray. Things break without this step.
    windows_volume::initialize_com().context("COM initialization failed")?;

    println!("COM initialized");

    windows_eco_mode::set_eco_mode_for_current_process()
        .unwrap_or_else(|err| println!("info: Failed to set Process mode to Eco: {}", err));

    let observer = windows_volume::VolumeObserver::from_device_name("voicemeeter input")?;
    let mut windows_volume_stream = observer.subscribe();

    let mut link = voicemeeter::Link::new().context("Failed to register with Voicemeeter")?;

    let exit_signal = Arc::new(Notify::new());

    {
        let exit_signal = exit_signal.to_owned();

        std::thread::Builder::new()
            .name("Tray icon event loop".into())
            .spawn(move || {
                let _ = (|| -> anyhow::Result<()> {
                    #[derive(Default)]
                    struct Application {
                        tray_icon: Option<tray_icon::TrayIcon>,
                    }

                    impl Application {
                        fn new_tray_menu(&mut self) -> anyhow::Result<()> {
                            let tray_menu = tray_icon::menu::Menu::new();
                            tray_menu
                                .append(&tray_icon::menu::MenuItem::new("Exit", true, None))
                                .context("Couldn't add item to the tray menu")?;

                            let tray_icon = tray_icon::TrayIconBuilder::new()
                                .with_menu(Box::new(tray_menu))
                                .with_tooltip(package_name!())
                                .with_icon(
                                    tray_icon::Icon::from_resource(1, None)
                                        .context("Failed to read icon resource")?,
                                )
                                .build()
                                .context("Failed to build tray icon")?;

                            self.tray_icon = Some(tray_icon);

                            Ok(())
                        }
                    }

                    impl winit::application::ApplicationHandler<tray_icon::menu::MenuEvent> for Application {
                        fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

                        fn window_event(
                            &mut self,
                            _event_loop: &winit::event_loop::ActiveEventLoop,
                            _window_id: winit::window::WindowId,
                            _event: winit::event::WindowEvent
                        ) {}

                        fn new_events(
                            &mut self,
                            _event_loop: &winit::event_loop::ActiveEventLoop,
                            cause: winit::event::StartCause,
                        ) {
                            // We create the icon once the event loop is actually running
                            // to prevent issues like https://github.com/tauri-apps/tray-icon/issues/90
                            if cause == winit::event::StartCause::Init {
                                let _ = self.new_tray_menu().inspect_err(print_error);
                            }
                        }

                        fn user_event(
                            &mut self,
                            event_loop: &winit::event_loop::ActiveEventLoop,
                            event: tray_icon::menu::MenuEvent
                        ) {
                            println!("{event:?}");
                            event_loop.exit();
                        }
                    }

                    use winit::event_loop::EventLoop;
                    use winit::platform::windows::EventLoopBuilderExtWindows;

                    let event_loop = EventLoop::<tray_icon::menu::MenuEvent>::with_user_event()
                        .with_any_thread(true)
                        .build()?;

                    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

                    let proxy = event_loop.create_proxy();
                    tray_icon::menu::MenuEvent::set_event_handler(Some(move |event| {
                        let _ = proxy.send_event(event);
                    }));

                    event_loop.run_app(&mut Application::default())?;

                    Ok(())
                })()
                .inspect_err(print_error);

                exit_signal.notify_one();
            })?;
    }

    let _ = (async {
        tokio_graceful::default_signal().await;
        anyhow::Ok(())
    })
    .or(async {
        exit_signal.notified().await;
        anyhow::Ok(())
    })
    .or(async {
        println!("Listening for volume changes");

        loop {
            windows_volume_stream
                .changed()
                .await
                .context("windows_volume_stream error 🤨")?;

            let volume_state = { windows_volume_stream.borrow().to_owned() };
            let gain = (-60.0).lerp(0.0, volume_state.volume);

            link.wait_for_connection().await;
            // Update the remote's cached program type so that it can
            // correctly find the needed strip(s)
            link.remote.update_program()?;

            let vm_strip = link.virtual_inputs().next().context(
                "There should absolutely be at least one \
                Virtual Input in any Voicemeeter edition \
                but it's not there 🤷.",
            )?;

            vm_strip
                .gain()
                .set(gain)
                .context("Couldn't set Voicemeeter strip gain")?;

            vm_strip
                .mute()
                .set(volume_state.is_muted)
                .context("Couldn't set Voicemeeter strip mute")?;

            println!("Changed to {volume_state:?} -> {gain:.1}db");
        }
    })
    .await
    .inspect_err(print_error);

    Ok(())
}
