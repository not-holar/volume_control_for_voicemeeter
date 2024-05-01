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

* Instead of constantly polling the volume slider for changes, this app uses the built-in Windows' `IAudioEndpointVolumeCallback` interface, thanks to which, the program is **completely idle** and not using **any** CPU resources when the volume isn't being changed.
* Tracks the volume changes of specifically the `VoiceMeeter Input` audio device instead of the typical approach of tracking whatever the default windows audio device is, meaning that **VoiceMeeter's input volume will be properly synced even when it *isn't* set as default** (useful when outputting audio from different applications to different devices)
* Tiny footprint (**0%** CPU, **.5mb** RAM).
* The application properly unregisters with Voicemeeter's API when exiting, which prevents leaking resources and causing visual weirdness in the Voicemeeter's GUI.
* Tray icon visually consistant with that of Voicemeeter.

## Download

Simply download the archive, which contains the **exe**, for your corresponding platform from the latest of [Releases](https://github.com/not-holar/volume_control_for_voicemeeter/releases)

* **x86_64-pc-windows-msvc** builds are for x86 CPUs (Intel, AMD)
* **aarch64-pc-windows-msvc** builds are for ARM CPUs (Qualcomm, etc) - please note that ARM builds are experimental, kindly [report any issues](https://github.com/not-holar/volume_control_for_voicemeeter/issues/new)

## Installation

This application is portable and lives entirely within it's .exe

To make the application start with Windows:

1. Open **File Explorer**
2. Paste **`shell:startup`** into the Address bar and press **Enter**
3. Put **volume_control_for_voicemeeter.exe** (or a shortcut to it) in the folder that opens
4. Launch the **exe** if it isn't running already

## Contributing

Please report any bugs you encounter as well as suggest improvements by using [Github issues](https://github.com/not-holar/volume_control_for_voicemeeter/issues).

Feel free to file Pull requests with your contributions

## Troubleshooting

To see the log of what is going on, exit the application and relaunch it inside of a terminal
