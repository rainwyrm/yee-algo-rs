/// how field magnitude is mapped to color intensity
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    /// intensity is proportional to `|E_z|`
    Linear,
    /// intensity is proportional to `sqrt(|E_z|)`. lifts weak fields
    /// (scattered and diffracted waves) out of the black
    /// without saturating the main pulse
    Sqrt,
}

impl Scale {
    /// the other scale, for a toggle key
    pub fn toggled(self) -> Self {
        match self {
            Scale::Linear => Scale::Sqrt,
            Scale::Sqrt => Scale::Linear,
        }
    }
}

/// color used for cells inside the obstacle
pub const OBSTACLE_RGB: [u8; 3] = [110, 110, 110];

/// maps a field value to a diverging red/black/blue color
///
/// the value is multiplied by `gain` and clamped to `[-1, 1]`; its magnitude
/// (optionally passed through the `sqrt` of [`Scale::Sqrt`]) sets the channel
/// intensity:
///
/// * `v >= 0` -> `[255 * v, 0, 0]` (red)
/// * `v <  0` -> `[0, 0, 255 * |v|]` (blue)
///
/// so `0` is black and `+- 1` is full red/blue. a `NaN` maps to black
pub fn signed_rgb(value: f32, gain: f32, scale: Scale) -> [u8; 3] {
    let v = (value * gain).clamp(-1.0, 1.0);
    let m = match scale {
        Scale::Linear => v.abs(),
        Scale::Sqrt => v.abs().sqrt(),
    };
    let c = (m * 255.0).round() as u8;
    if v >= 0.0 { [c, 0, 0] } else { [0, 0, c] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_black_and_extremes_are_saturated() {
        assert_eq!(signed_rgb(0.0, 1.0, Scale::Linear), [0, 0, 0]);
        assert_eq!(signed_rgb(1.0, 1.0, Scale::Linear), [255, 0, 0]);
        assert_eq!(signed_rgb(-1.0, 1.0, Scale::Linear), [0, 0, 255]);
    }

    #[test]
    fn gain_scales_and_clamps() {
        assert_eq!(signed_rgb(0.25, 2.0, Scale::Linear), [128, 0, 0]);
        assert_eq!(signed_rgb(10.0, 1.0, Scale::Linear), [255, 0, 0]);
        assert_eq!(signed_rgb(-10.0, 1.0, Scale::Linear), [0, 0, 255]);
    }

    #[test]
    fn sqrt_lifts_weak_fields() {
        let lin = signed_rgb(0.04, 1.0, Scale::Linear)[0];
        let sq = signed_rgb(0.04, 1.0, Scale::Sqrt)[0];
        assert_eq!((lin, sq), (10, 51));
        // sign is preserved
        assert_eq!(signed_rgb(-0.04, 1.0, Scale::Sqrt), [0, 0, 51]);
    }

    #[test]
    fn nan_is_black() {
        assert_eq!(signed_rgb(f32::NAN, 1.0, Scale::Linear), [0, 0, 0]);
    }

    #[test]
    fn scale_toggles_back_and_forth() {
        assert_eq!(Scale::Linear.toggled().toggled(), Scale::Linear);
    }
}
