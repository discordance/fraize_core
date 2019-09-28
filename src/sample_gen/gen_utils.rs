use std::f32;

/// A clamped fade_in
pub fn fade_in(t: i64, len: i64) -> f32 {
    // (t as f32 / len as f32).exp()
    let c1 = f32::consts::E.powf(t as f32 / len as f32) - 1.0;
    let c2 = f32::consts::E - 1.0;
    let r = c1 / c2;
    if r < 1.0 {
        return r;
    }
    return 1.0;
}

/// a clamped fade out
/// @TODO this env does not sound good for basses
pub fn fade_out(t: i64, len: i64, end: i64) -> f32 {
    let c1 = f32::consts::E.powf((end as f32 - t as f32) / len as f32) - 1.0;
    let c2 = f32::consts::E - 1.0;
    let r = c1 / c2;
    if r < 1.0 {
        return r;
    }
    return 1.0;
}
