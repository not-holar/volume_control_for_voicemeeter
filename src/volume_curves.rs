use lerp::num_traits::clamp;
use libm::*;

pub fn ease_out_expo(value: f32) -> f32{
    let normalized = clamp(value, 0.0, 1.0);
    if normalized == 1.0 {
        1.0
    }
    else {
        1.0 - powf(2.0, -5.0 * normalized)
    }
}