mod voicemeeter;
mod windows_eco_mode;
mod windows_volume;

use anyhow::Context;
use lerp::Lerp;
use smol::future::FutureExt;

fn print_error(err: impl std::fmt::Display) {
    println!("{err}");
}

fn handle_the_error(err: impl std::fmt::Display) {
    print_error(err);

    // println!("\nPress ENTER to continue...");
    // std::io::stdin().lines().next();
}

fn main() {
    smol::block_on(listen()).unwrap_or_else(handle_the_error);

    println!("Exiting safely");
}

async fn listen() -> anyhow::Result<()> {
    // Initialize Win32's COM libray. Things break without this step.
    windows_volume::initialize_com().context("COM initialization failed")?;

    windows_eco_mode::set_eco_mode_for_current_process()
        .unwrap_or_else(|err| println!("Failed to set Process mode to Eco: {}", err));

    let observer = windows_volume::VolumeObserver::from_device_name("voicemeeter vaio")?;
    let windows_volume_stream = observer.subscribe();

    let link = voicemeeter::Link::new().context("Failed to register with Voicemeeter")?;

    (async {
        tokio_graceful::default_signal().await;
        anyhow::Ok(())
    })
    .or(async {
        let mut previous_vm_edition = ::voicemeeter::types::VoicemeeterApplication::None;
        let mut vm_gain_parameter = None;

        loop {
            // linear position of the volume slider from 0.0 to 1.0
            let volume_slider_position = windows_volume_stream
                .recv()
                .await
                .context("windows_volume_stream error 🤨")?;

            link.wait_for_connection().await;

            {
                let mut remote = link.inner.remote.lock().await;

                let vm_edition = {
                    remote.update_program()?;
                    remote.program
                };

                if vm_edition != previous_vm_edition {
                    previous_vm_edition = vm_edition;
                    vm_gain_parameter = Some({
                        let strip = voicemeeter::Link::virtual_inputs(&remote).next().context(
                            "There should absolutely be at least one \
                            Virtual Input in any Voicemeeter edition \
                            but it's not there 🤷.",
                        )?;

                        link.gain_parameter_of(&strip)
                    });
                }
            };
            let vm_gain_parameter = vm_gain_parameter.as_ref().unwrap();

            let gain = (-60.0).lerp(0.0, volume_slider_position);

            vm_gain_parameter
                .set(gain)
                .await
                .context("Couldn't set slider value")
                .unwrap_or_else(print_error)
        }
    })
    .await
    .unwrap_or_else(print_error);

    Ok(())
}
