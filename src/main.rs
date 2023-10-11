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

    let voicemeeter_gain_parameter = {
        link.wait_for_connection().await;

        let strip = link.virtual_inputs().next().context(
            "There should absolutely be at least one \
            Virtual Input in any Voicemeeter edition \
            but it's not there 🤷.",
        )?;

        link.gain_parameter_of(&strip)
    };

    (async {
        tokio_graceful::default_signal().await;
        anyhow::Ok(())
    })
    .or(async {
        loop {
            // linear position of the volume slider from 0.0 to 1.0
            let volume_slider_position = windows_volume_stream
                .recv()
                .await
                .context("windows_volume_stream error 🤨")?;

            let gain = (-60.0).lerp(0.0, volume_slider_position);

            voicemeeter_gain_parameter
                .set(gain)
                .context("Couldn't set slider value")
                .unwrap_or_else(print_error)
        }
    })
    .await
    .unwrap_or_else(print_error);

    Ok(())
}
