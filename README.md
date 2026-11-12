<p align="center">
    <img width="150" 
        src="media/icon.svg" 
        alt="logo">
</p>

<h1 align="center">volume_control_for_voicemeeter</h1>

A simple and tiny tray application that syncs the **in-Windows** volume of the `VoiceMeeter Input (VAIO)` audio output device to the volume of the first virtual input strip **in Voicemeeter** (which corresponds to that same device).

This allows the built-in Windows volume slider 🎚️, any mixer apps' (e.g. sndvol.exe, EarTrumpet, etc) sliders 🎚️ as well as gestures and keyboard volume keys ⌨️ to control the volume of Voicemeeter.

Written in Rust  🦀

[![Deploy](https://github.com/not-holar/volume_control_for_voicemeeter/actions/workflows/rust.yml/badge.svg)](https://github.com/not-holar/volume_control_for_voicemeeter/actions/workflows/rust.yml)

### Features

* Instead of constantly polling the volume slider for changes, this app uses the built-in Windows' `IAudioEndpointVolumeCallback` interface, so volume changes are applied instantly. Apart from that, it only does a tiny check every 2 seconds to notice Voicemeeter or the audio device going away and coming back.
* **Self-healing**: it can be started before Voicemeeter (e.g. at logon, no startup delay needed) and keeps working when Voicemeeter or its audio engine is restarted. It also watches for audio device changes and re-attaches when Windows re-creates the `VoiceMeeter Input` device. Transient errors are logged and retried instead of closing the app.
* **Boost above 0 dB**: right-click the tray icon → **Volume at 100%** → `0 dB` (default), `+3 dB`, `+6 dB` or `+12 dB`. With a boost selected, 0 dB always sits at 80% of the Windows slider and the last 20% goes up to the chosen boost. The choice is remembered.
* Tracks the volume changes of specifically the `VoiceMeeter Input` audio device instead of the typical approach of tracking whatever the default windows audio device is, meaning that **VoiceMeeter's input volume will be properly synced even when it *isn't* set as default** (useful when outputting audio from different applications to different devices)
* Tiny footprint (**~0%** CPU, a few MB of RAM).
* The application properly unregisters with Voicemeeter's API when exiting, which prevents leaking resources and causing visual weirdness in the Voicemeeter's GUI.
* Tray icon visually consistant with that of Voicemeeter.

## Download

Simply download the **zip** archive (**exe** inside) for your corresponding platform from the [latest](https://github.com/not-holar/volume_control_for_voicemeeter/releases/latest) of [Releases](https://github.com/not-holar/volume_control_for_voicemeeter/releases)

* **x86_64-pc-windows-msvc** builds are for x86 CPUs (Intel, AMD)
* **aarch64-pc-windows-msvc** builds are for ARM CPUs (Qualcomm, etc) - please note that ARM builds are experimental, kindly [report any issues](https://github.com/not-holar/volume_control_for_voicemeeter/issues/new)

## Installation

This application is portable and lives entirely within it's .exe

To make the application start with Windows:

1. Open **File Explorer**
2. Paste **`shell:startup`** into the Address bar and press **Enter**
3. Put **volume_control_for_voicemeeter.exe** (or a shortcut to it) in the folder that opens
4. Launch the **exe** if it isn't running already

There is no need to delay the start (Task Scheduler etc.): the app waits for Voicemeeter and the audio device by itself.

## Contributing

Please report any bugs you encounter as well as suggest improvements by using [Github issues](https://github.com/not-holar/volume_control_for_voicemeeter/issues).

Feel free to file Pull requests with your contributions

## Troubleshooting

Right-click the tray icon → **Open log** to see what is going on. The log is stored in `%TEMP%\volume_control_for_voicemeeter.log` (the previous one is kept as `volume_control_for_voicemeeter.old.log`). Launching the application from a terminal also prints the log there.

The "Volume at 100%" setting is stored in `%APPDATA%\volume_control_for_voicemeeter\max_gain_db.txt`.
