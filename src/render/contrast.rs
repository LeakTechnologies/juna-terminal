//! Presentation-only accessibility policy; engine colors are never modified.
//! sRGB transfer and relative luminance: W3C WCAG 2.2, contrast minimum.
pub const DEFAULT_MINIMUM: f32 = 4.5;
pub fn linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}
fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * linear(rgb[0]) + 0.7152 * linear(rgb[1]) + 0.0722 * linear(rgb[2])
}
pub fn ratio(a: [f32; 3], b: [f32; 3]) -> f32 {
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
pub fn foreground(fg: [f32; 3], bg: [f32; 3], minimum: f32) -> [f32; 3] {
    if minimum <= 1. || ratio(fg, bg) >= minimum {
        return fg;
    }
    let endpoint = if ratio([0.; 3], bg) >= ratio([1.; 3], bg) {
        [0.; 3]
    } else {
        [1.; 3]
    };
    if ratio(endpoint, bg) < minimum {
        return endpoint;
    }
    // Move toward the best endpoint only as far as required, retaining hue when possible.
    let mut low = 0.;
    let mut high = 1.;
    for _ in 0..24 {
        let weight = (low + high) * 0.5;
        let candidate = std::array::from_fn(|i| fg[i] + (endpoint[i] - fg[i]) * weight);
        if ratio(candidate, bg) >= minimum {
            high = weight;
        } else {
            low = weight;
        }
    }
    std::array::from_fn(|i| fg[i] + (endpoint[i] - fg[i]) * high)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn light_boxes_get_dark_readable_text_without_changing_the_background() {
        for bg in [
            [189. / 255., 147. / 255., 249. / 255.],
            [1., 121. / 255., 198. / 255.],
            [0.94, 0.98, 0.55],
        ] {
            let fg = foreground([0.97, 0.97, 0.95], bg, DEFAULT_MINIMUM);
            assert!(ratio(fg, bg) >= 4.499);
            assert!(luminance(fg) < luminance(bg));
        }
    }
    #[test]
    fn good_colors_and_disabled_correction_are_preserved() {
        let fg = [0.97, 0.97, 0.95];
        let bg = [40. / 255., 42. / 255., 54. / 255.];
        assert_eq!(foreground(fg, bg, DEFAULT_MINIMUM), fg);
        assert_eq!(foreground(fg, [0.95; 3], 1.), fg);
        assert!((ratio([0.; 3], [1.; 3]) - 21.).abs() < 0.001);
    }
}
