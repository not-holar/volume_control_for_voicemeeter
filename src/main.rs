// mod eco_mode;
mod voicemeeter;
mod windows_volume;

use lerp::Lerp;

fn handle_the_error(err: String) {
    println!("{err}");

    println!("\nPress ENTER to continue...");
    std::io::stdin().lines().next();
}

#[tokio::main]
async fn main() {
    listen().await.unwrap_or_else(handle_the_error);
}

async fn listen() -> Result<(), String> {
    // Initialize Win32's COM libray. Things break without this step.
    windows_volume::initialize_com().map_err(|err| format!("COM initialization failed: {err}"))?;

    // eco_mode::set_eco_mode_for_current_process()
    //     .unwrap_or_else(|err| println!("Failed to set Process mode to Eco: {}", err));

    let voicemeeter_gain_parameter = {
        let link = loop {
            match voicemeeter::Link::new() {
                Err(err) => {
                    println!("Failed to connect to Voicemeeter: {err}\nRetrying in 15s");
                    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                }
                Ok(link) => break link,
            }
        };
        let strip = link.virtual_inputs().nth(0).ok_or(
            concat!(
                "There should absolutely be at least one",
                " Virtual Input in any Voicemeeter edition",
                " but it's not there 🤷."
            )
            .to_string(),
        )?;
        link.gain_parameter(&strip)
    };

    let observer = windows_volume::VolumeObserver::from_device_name("voicemeeter vaio")?;
    let mut rx = observer.subscribe();

    loop {
        // linear position of the volume slider from 0.0 to 1.0
        let volume_slider_position = rx
            .recv()
            .await
            .map_err(|err| format!("Stream error: {err:#?}"))?;

        let gain = (-60.0).lerp(0.0, volume_slider_position);

        voicemeeter_gain_parameter
            .set(gain)
            .unwrap_or_else(|err| println!("Couldn't set slider value: {err:?}"))
    }
}
