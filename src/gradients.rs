pub const NAMES: [&str; 13] = [
    "Generated",
    "Ember",
    "Ocean",
    "Aurora",
    "Sunset",
    "Orchid",
    "Forest",
    "Ice",
    "Copper",
    "Candy",
    "Monochrome",
    "Solar",
    "Twilight",
];
const COLOURS: [[[u8; 3]; 3]; 12] = [
    [[80, 8, 35], [255, 65, 10], [255, 238, 130]],
    [[12, 24, 100], [0, 150, 215], [140, 255, 235]],
    [[45, 5, 105], [0, 220, 130], [195, 255, 75]],
    [[55, 15, 115], [245, 65, 100], [255, 205, 90]],
    [[35, 8, 85], [170, 30, 210], [255, 190, 240]],
    [[5, 40, 25], [30, 170, 70], [220, 245, 100]],
    [[20, 50, 120], [85, 195, 245], [235, 255, 255]],
    [[55, 15, 12], [180, 80, 35], [255, 215, 155]],
    [[40, 70, 220], [255, 80, 180], [110, 255, 230]],
    [[35, 35, 35], [145, 145, 145], [255, 255, 255]],
    [[120, 15, 0], [255, 150, 0], [255, 255, 175]],
    [[10, 15, 70], [85, 70, 180], [245, 160, 105]],
];
pub const SIZE: usize = 256;
pub type Palette = [[f32; 4]; SIZE];
pub fn palette(id: usize) -> Palette {
    let stops = COLOURS[id.saturating_sub(1).min(COLOURS.len() - 1)];
    std::array::from_fn(|i| {
        let t = i as f32 / (SIZE - 1) as f32 * 2.;
        let index = (t as usize).min(1);
        let f = t - index as f32;
        std::array::from_fn(|c| {
            if c == 3 {
                1.
            } else {
                (stops[index][c] as f32 * (1. - f) + stops[index + 1][c] as f32 * f) / 255.
            }
        })
    })
}
fn hsv(h: f32, s: f32, v: f32) -> [f32; 4] {
    let h = h.rem_euclid(1.) * 6.;
    let i = h.floor() as u32;
    let f = h - h.floor();
    let p = v * (1. - s);
    let q = v * (1. - s * f);
    let t = v * (1. - s * (1. - f));
    let rgb = match i % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    };
    [rgb[0], rgb[1], rgb[2], 1.]
}
pub fn random(seed: u32) -> Palette {
    let mut state = seed.max(1);
    let mut unit = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state >> 8) as f32 / 16777216.
    };
    let count = 16 + (unit() * 17.) as usize;
    let mut stops = Vec::new();
    let mut position = 0.;
    let mut hue = unit();
    for i in 0..count {
        hue = (hue + 0.13 + unit() * 0.65) % 1.;
        let saturation = 0.65 + unit() * 0.35;
        let value = if i % 7 == 0 {
            0.06 + unit() * 0.15
        } else {
            0.55 + unit() * 0.45
        };
        stops.push((position, hsv(hue, saturation, value)));
        position += 0.25 + unit() * 1.75;
    }
    let end = stops.last().unwrap().0;
    std::array::from_fn(|i| {
        let t = i as f32 / (SIZE - 1) as f32 * end;
        let index = stops
            .partition_point(|(p, _)| *p <= t)
            .saturating_sub(1)
            .min(count - 2);
        let f = ((t - stops[index].0) / (stops[index + 1].0 - stops[index].0)).clamp(0., 1.);
        std::array::from_fn(|c| stops[index].1[c] * (1. - f) + stops[index + 1].1[c] * f)
    })
}
pub fn rotated(palette: &Palette, rotation: usize) -> Palette {
    std::array::from_fn(|i| palette[(i + rotation) % SIZE])
}
pub fn sample(palette: &Palette, position: f32) -> [f32; 3] {
    let t = position.clamp(0., 1.) * (SIZE - 1) as f32;
    let index = (t as usize).min(SIZE - 2);
    let f = t - index as f32;
    std::array::from_fn(|c| palette[index][c] * (1. - f) + palette[index + 1][c] * f)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gradients_have_distinct_colours_and_valid_endpoints() {
        for id in 1..NAMES.len() {
            let p = palette(id);
            assert_eq!(sample(&p, 0.), [p[0][0], p[0][1], p[0][2]]);
            assert_eq!(sample(&p, 1.), [p[255][0], p[255][1], p[255][2]]);
            assert_ne!(p[0], p[255]);
        }
    }
    #[test]
    fn random_palettes_are_repeatable_colourful_and_rotate_losslessly() {
        let p = random(123);
        assert_eq!(p, random(123));
        assert_ne!(p, random(456));
        assert!(p
            .iter()
            .flatten()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        for c in 0..3 {
            assert!(p.iter().any(|v| v[c] > 0.7));
        }
        let transitions = p
            .windows(2)
            .filter(|pair| (0..3).any(|c| (pair[0][c] - pair[1][c]).abs() > 0.03))
            .count();
        assert!(transitions > 80);
        assert_eq!(rotated(&rotated(&p, 37), 256 - 37), p);
    }
}
