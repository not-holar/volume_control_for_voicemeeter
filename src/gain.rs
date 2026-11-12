//! Mapping of the Windows volume slider (0.0 ..= 1.0) to a Voicemeeter strip gain in dB.

/// Gain at 0% volume.
pub const MIN_GAIN_DB: f32 = -60.0;
/// The highest gain a Voicemeeter strip accepts.
pub const MAX_ALLOWED_GAIN_DB: f32 = 12.0;
/// Slider position of 0 dB whenever boosting above 0 dB is enabled.
pub const UNITY_POSITION: f32 = 0.8;

/// Converts a Windows volume scalar into a Voicemeeter gain.
///
/// * `max_gain_db <= 0`: `0% ..= 100%` maps linearly onto `-60 ..= 0 dB` (the classic behaviour).
/// * `max_gain_db > 0`: `0% ..= 80%` maps onto `-60 ..= 0 dB` and `80% ..= 100%` onto
///   `0 ..= max_gain_db`, so 0 dB always sits at 80% no matter which maximum is chosen.
///
/// The result is rounded to 0.1 dB, the precision Voicemeeter displays.
pub fn volume_to_gain(volume: f32, max_gain_db: f32) -> f32 {
    let volume = volume.clamp(0.0, 1.0);
    let max_gain_db = max_gain_db.clamp(0.0, MAX_ALLOWED_GAIN_DB);

    let unity = if max_gain_db > 0.0 { UNITY_POSITION } else { 1.0 };

    let gain = if volume <= unity {
        MIN_GAIN_DB * (1.0 - volume / unity)
    } else {
        max_gain_db * (volume - unity) / (1.0 - unity)
    };

    // `+ 0.0` turns a possible -0.0 into 0.0
    (gain * 10.0).round() / 10.0 + 0.0
}

#[cfg(test)]
mod tests {
    use super::volume_to_gain;

    #[track_caller]
    fn check(volume: f32, max: f32, expected: f32) {
        let gain = volume_to_gain(volume, max);
        assert!(
            (gain - expected).abs() < 0.051,
            "volume_to_gain({volume}, {max}) = {gain}, expected {expected}"
        );
    }

    #[test]
    fn classic_mapping_without_boost() {
        check(0.0, 0.0, -60.0);
        check(0.5, 0.0, -30.0);
        check(0.94, 0.0, -3.6);
        check(1.0, 0.0, 0.0);
    }

    #[test]
    fn unity_is_fixed_at_80_percent_with_boost() {
        for max in [3.0, 6.0, 12.0] {
            check(0.0, max, -60.0);
            check(0.5, max, -22.5);
            check(0.8, max, 0.0);
            check(1.0, max, max);
        }
        check(0.9, 12.0, 6.0);
        check(0.9, 6.0, 3.0);
    }

    #[test]
    fn out_of_range_inputs_are_clamped() {
        check(-0.5, 12.0, -60.0);
        check(1.5, 12.0, 12.0);
        check(1.0, 40.0, 12.0);
        check(1.0, -10.0, 0.0);
    }

    #[test]
    fn never_returns_negative_zero() {
        assert!(volume_to_gain(0.8, 12.0).is_sign_positive());
        assert!(volume_to_gain(1.0, 0.0).is_sign_positive());
    }
}
