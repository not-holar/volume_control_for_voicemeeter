//! Persisted user settings, stored in `%APPDATA%\volume_control_for_voicemeeter\`.

use std::path::PathBuf;

/// Choices offered in the tray menu for the gain at 100% volume.
pub const MAX_GAIN_CHOICES: [f32; 4] = [0.0, 3.0, 6.0, 12.0];

fn max_gain_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| {
        PathBuf::from(dir)
            .join(env!("CARGO_PKG_NAME"))
            .join("max_gain_db.txt")
    })
}

/// The saved max gain, or 0 dB when nothing (valid) was saved yet.
pub fn load_max_gain() -> f32 {
    max_gain_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| text.trim().parse::<f32>().ok())
        .filter(|db| MAX_GAIN_CHOICES.contains(db))
        .unwrap_or(0.0)
}

pub fn save_max_gain(db: f32) -> std::io::Result<()> {
    let path = max_gain_path().ok_or(std::io::ErrorKind::NotFound)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, db.to_string())
}

pub fn max_gain_label(db: f32) -> String {
    if db > 0.0 {
        format!("+{db} dB")
    } else {
        "0 dB".to_string()
    }
}
