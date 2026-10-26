use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Transform {
    pub row0: [f32; 4],
    pub row1: [f32; 4],
    pub variation: [f32; 4], // kind, nonlinear blend, colour coordinate, unused
    pub parameters: [f32; 4],
}
#[derive(Clone)]
pub struct Flame {
    pub seed: u32,
    pub transforms: [Transform; 4],
    pub camera: [f32; 4],
    pub palette: crate::gradients::Palette,
}
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Params {
    pub camera: [f32; 4],
    pub size: [u32; 4],
    pub palette: crate::gradients::Palette,
    pub transforms: [Transform; 4],
}
impl Flame {
    pub fn params(&self, width: u32, height: u32) -> Params {
        Params {
            camera: self.camera,
            size: [width, height, 0, 0],
            palette: self.palette,
            transforms: self.transforms,
        }
    }
}
pub struct Random(pub u32);
impl Random {
    pub fn next(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / 16777216.
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}
pub fn apply(t: &Transform, p: [f32; 2]) -> [f32; 2] {
    apply_random(t, p, [0.17, 0.31, 0.53, 0.83, 0.23])
}
pub fn apply_random(t: &Transform, mut p: [f32; 2], noise: [f32; 5]) -> [f32; 2] {
    let q = t.parameters;
    let gaussian = noise[..4].iter().sum::<f32>() - 2.;
    let tau = std::f32::consts::TAU;
    if t.variation[0] as u32 == 29 {
        let a = noise[4] * tau;
        let amount = q[0] * gaussian;
        p[0] += amount * a.cos();
        p[1] += amount * a.sin();
    }
    let x = t.row0[0] * p[0] + t.row0[1] * p[1] + t.row0[2];
    let y = t.row1[0] * p[0] + t.row1[1] * p[1] + t.row1[2];
    let r2 = (x * x + y * y).max(0.03);
    let r = r2.sqrt();
    let radius = (x * x + y * y).sqrt();
    let safe_r = radius.max(1e-6);
    let angle = if radius > 1e-6 { x.atan2(y) } else { 0. };
    let pi = std::f32::consts::PI;
    let v = match t.variation[0] as u32 {
        1 => [x.sin(), y.sin()],
        2 => [x / r2, y / r2],
        3 => [x * r2.sin() - y * r2.cos(), x * r2.cos() + y * r2.sin()],
        4 => [(x - y) * (x + y) / r, 2. * x * y / r],
        5 => [angle / pi, radius - 1.],
        6 => [
            radius * (angle + radius).sin(),
            radius * (angle - radius).cos(),
        ],
        7 => [
            radius * (angle * radius).sin(),
            -radius * (angle * radius).cos(),
        ],
        8 => [
            angle / pi * (pi * radius).sin(),
            angle / pi * (pi * radius).cos(),
        ],
        9 => [
            (angle.cos() + radius.sin()) / safe_r,
            (angle.sin() - radius.cos()) / safe_r,
        ],
        10 => [angle.sin() / safe_r, radius * angle.cos()],
        11 => [angle.sin() * radius.cos(), angle.cos() * radius.sin()],
        12 => {
            let a = (angle + radius).sin().powi(3);
            let b = (angle - radius).cos().powi(3);
            [radius * (a + b), radius * (a - b)]
        }
        13 => [
            if x < 0. { 2. * x } else { x },
            if y < 0. { y * 0.5 } else { y },
        ],
        14 => [2. * y / (radius + 1.), 2. * x / (radius + 1.)],
        15 => [2. * x / (radius + 1.), 2. * y / (radius + 1.)],
        16 => [
            4. * x / (radius * radius + 4.),
            4. * y / (radius * radius + 4.),
        ],
        17 => [x.sin(), y],
        18 => {
            let a = angle * 0.5 + if noise[0] < 0.5 { 0. } else { pi };
            let rr = radius.sqrt();
            [rr * a.cos(), rr * a.sin()]
        }
        19 => [
            x + q[0] * (y / (q[2] * q[2] + 1e-6)).sin(),
            y + q[1] * (x / (q[3] * q[3] + 1e-6)).sin(),
        ],
        20 => [
            x + q[0] * (3. * y).tan().sin(),
            y + q[1] * (3. * x).tan().sin(),
        ],
        21 => {
            let rr = (x - 1.).clamp(-40., 40.).exp();
            [rr * (pi * y).cos(), rr * (pi * y).sin()]
        }
        22 => {
            let rr = safe_r.powf(angle.sin());
            [rr * angle.cos(), rr * angle.sin()]
        }
        23 => [
            (pi * x).cos() * y.clamp(-40., 40.).cosh(),
            -(pi * x).sin() * y.clamp(-40., 40.).sinh(),
        ],
        24 => {
            let d = t.row0[2] * t.row0[2] + 1e-6;
            let rr = (radius + d) % (2. * d) - d + radius * (1. - d);
            [rr * angle.cos(), rr * angle.sin()]
        }
        25 => {
            let d = pi * (t.row0[2] * t.row0[2] + 1e-6);
            let a = angle
                + if (angle + t.row1[2]) % d > d * 0.5 {
                    -d * 0.5
                } else {
                    d * 0.5
                };
            [radius * a.cos(), radius * a.sin()]
        }
        26 => {
            let a = noise[0] * tau;
            [x * noise[1] * a.cos(), y * noise[1] * a.sin()]
        }
        27 => {
            let a = noise[0] * tau;
            [noise[1] * a.cos(), noise[1] * a.sin()]
        }
        28 => {
            let a = noise[4] * tau;
            [gaussian * a.cos(), gaussian * a.sin()]
        }
        29 => [x, y],
        30 => {
            let a = y.atan2(x) + (q[0] * pi * 0.5).sin() * gaussian;
            let z = (q[0] * pi * 0.5).cos() * gaussian - 1.;
            [radius * a.cos() + z * x, radius * a.sin() + z * y]
        }
        31 => {
            let d = q[0] * q[0] + 1e-6;
            let rr = radius - 2. * d * ((radius + d) / (2. * d)).trunc() + radius * (1. - d);
            [angle.sin() * rr, angle.cos() * rr]
        }
        32 => {
            let d = pi * (q[0] * q[0] + 1e-6);
            let a = angle
                + if angle + q[1] - d * ((angle + q[1]) / d).trunc() > d * 0.5 {
                    -d * 0.5
                } else {
                    d * 0.5
                };
            [radius * a.sin(), radius * a.cos()]
        }
        33 => {
            let rr = radius * (q[0] + (q[1] - q[0]) * (0.5 + 0.5 * (q[2] * angle).sin()));
            [rr * angle.sin(), rr * angle.cos()]
        }
        34 => [
            (q[0] * y).sin() - (q[1] * x).cos(),
            (q[2] * x).sin() - (q[3] * y).cos(),
        ],
        35 => {
            let a = q[0] * pi * 0.5;
            let denominator = q[1] - y * a.sin();
            let denominator = if denominator.abs() < 1e-6 {
                1e-6
            } else {
                denominator
            };
            [q[1] * x / denominator, q[1] * a.cos() * y / denominator]
        }
        36 | 37 => {
            let power = q[0].abs().round().max(1.);
            let branch = (noise[0] * power).floor();
            let theta = y.atan2(x)
                * if t.variation[0] as u32 == 37 && branch as u32 % 2 == 1 {
                    -1.
                } else {
                    1.
                };
            let a = (theta + tau * branch) / power;
            let rr = safe_r.powf(q[1] / power);
            [rr * a.cos(), rr * a.sin()]
        }
        38 => {
            let re = 1. + q[0] * x + q[1] * (x * x - y * y);
            let im = q[0] * y + 2. * q[1] * x * y;
            let d = (re * re + im * im).max(1e-6);
            [(x * re + y * im) / d, (y * re - x * im) / d]
        }
        39 => {
            let dx = q[0].abs().max(1e-6);
            let dy = q[1].abs().max(1e-6);
            [
                (2. * (x / dx).floor() + 1.) * dx - x,
                (2. * (y / dy).floor() + 1.) * dy - y,
            ]
        }
        _ => [x, y],
    };
    let blend = t.variation[1];
    [x + (v[0] - x) * blend, y + (v[1] - y) * blend]
}
fn candidate(seed: u32, options: &crate::settings::GenerationOptions) -> Flame {
    let mut rng = Random(seed);
    let allowed: Vec<usize> = (0..crate::settings::VARIATION_COUNT)
        .filter(|&i| options.enabled[i])
        .collect();
    let mut transforms = std::array::from_fn(|i| {
        let a = rng.range(0., std::f32::consts::TAU);
        let b = a + rng.range(-0.6, 0.6);
        let sx = rng.range(0.3, 0.95);
        let sy = rng.range(0.3, 0.95) * if rng.unit() < 0.3 { -1. } else { 1. };
        Transform {
            row0: [
                a.cos() * sx,
                -b.sin() * sy,
                rng.range(-1.1, 1.1),
                rng.range(0.3, 1.),
            ],
            row1: [a.sin() * sx, b.cos() * sy, rng.range(-1.1, 1.1), 0.],
            variation: [
                {
                    let choices: Vec<usize> = allowed
                        .iter()
                        .copied()
                        .filter(|&kind| i != 0 || kind != 0 || allowed.len() == 1)
                        .collect();
                    choices[(rng.next() as usize) % choices.len()] as f32
                },
                if options.enabled[0] {
                    rng.range(0.45, 1.)
                } else {
                    1.
                },
                rng.unit(),
                0.,
            ],
            parameters: [
                rng.range(0.2, 0.8),
                rng.range(0.3, 1.2),
                rng.range(0.3, 1.2),
                rng.range(0.3, 1.2),
            ],
        }
    });
    for t in &mut transforms {
        match t.variation[0] as u32 {
            29 => t.parameters[0] = rng.range(0.03, 0.3),
            33 => {
                t.parameters = [
                    rng.range(0.2, 0.7),
                    rng.range(0.8, 1.3),
                    rng.range(2., 7.).round(),
                    0.,
                ]
            }
            34 => t.parameters = std::array::from_fn(|_| rng.range(-2., 2.)),
            35 => t.parameters = [rng.range(-0.5, 0.5), rng.range(1.2, 3.), 0., 0.],
            36 | 37 => t.parameters = [(2 + rng.next() % 5) as f32, rng.range(0.5, 1.5), 0., 0.],
            38 => t.parameters = [rng.range(-0.7, 0.7), rng.range(-0.3, 0.3), 0., 0.],
            _ => (),
        }
    }
    let total: f32 = transforms.iter().map(|t| t.row0[3]).sum();
    let mut sum = 0.;
    for t in &mut transforms {
        sum += t.row0[3] / total;
        t.row1[3] = sum;
    }
    transforms[3].row1[3] = 1.;
    Flame {
        seed,
        transforms,
        camera: [0., 0., 5., 5.],
        palette: crate::gradients::random(seed ^ 0x91d32a47),
    }
}
// A cheap orbit probe rejects collapsed/escaping candidates and fits the view.
fn fit(flame: &mut Flame) -> bool {
    let mut rng = Random(flame.seed ^ 0xa53c91e7);
    let mut p = [0.; 2];
    let mut points = Vec::with_capacity(6000);
    for i in 0..6200 {
        let r = rng.unit();
        let t = flame.transforms.iter().find(|t| r < t.row1[3]).unwrap();
        p = apply_random(t, p, std::array::from_fn(|_| rng.unit()));
        if !p.iter().all(|v| v.is_finite() && v.abs() < 100.) {
            return false;
        }
        if i >= 200 {
            points.push(p);
        }
    }
    let mut xs: Vec<f32> = points.iter().map(|p| p[0]).collect();
    let mut ys: Vec<f32> = points.iter().map(|p| p[1]).collect();
    xs.sort_unstable_by(f32::total_cmp);
    ys.sort_unstable_by(f32::total_cmp);
    let lo = 30;
    let hi = points.len() - 31;
    let span = (xs[hi] - xs[lo]).max(ys[hi] - ys[lo]);
    if !(0.15..30.).contains(&span) {
        return false;
    }
    let cx = (xs[lo] + xs[hi]) * 0.5;
    let cy = (ys[lo] + ys[hi]) * 0.5;
    let extent = span * 1.18;
    let mut cells = [false; 32 * 32];
    for p in &points {
        let x = ((p[0] - cx) / extent + 0.5) * 32.;
        let y = ((p[1] - cy) / extent + 0.5) * 32.;
        if (0.0..32.).contains(&x) && (0.0..32.).contains(&y) {
            cells[y as usize * 32 + x as usize] = true;
        }
    }
    if cells.iter().filter(|&&c| c).count() < 65 {
        return false;
    }
    flame.camera = [cx, cy, extent, extent];
    true
}
pub fn generate(seed: u32) -> Flame {
    generate_with(seed, &crate::settings::GenerationOptions::default())
}
pub fn generate_with(seed: u32, options: &crate::settings::GenerationOptions) -> Flame {
    assert!(options.valid(), "At least one variation must be enabled");
    let mut rng = Random(seed);
    for _ in 0..64 {
        let mut flame = candidate(rng.next(), options);
        if fit(&mut flame) {
            return flame;
        }
    }
    // Deterministic known-good search avoids returning a blank candidate.
    let mut flame = candidate(13579, options);
    if !fit(&mut flame) {
        flame.camera = [0., 0., 5., 5.];
    }
    flame
}
pub fn colours<T: Copy + Into<u64>>(pixels: &[[T; 4]], exposure: f32) -> Vec<u8> {
    image_bytes(pixels, exposure, false)
}
pub fn image_bytes<T: Copy + Into<u64>>(
    pixels: &[[T; 4]],
    exposure: f32,
    transparent: bool,
) -> Vec<u8> {
    let mut densities: Vec<u64> = pixels
        .iter()
        .map(|p| p[0].into())
        .filter(|&n| n > 0)
        .collect();
    let reference = if densities.is_empty() {
        1
    } else {
        let index = densities.len() * 99 / 100;
        *densities.select_nth_unstable(index).1
    }
    .max(1) as f32;
    pixels
        .iter()
        .flat_map(|p| {
            if p[0].into() == 0 {
                return (if transparent {
                    [0, 0, 0, 0]
                } else {
                    [5, 7, 12, 255]
                })
                .into_iter()
                .take(if transparent { 4 } else { 3 });
            }
            let brightness = ((p[0].into() as f32).ln_1p() / reference.ln_1p() * exposure)
                .clamp(0., 1.)
                .powf(0.65);
            let rgb: [u8; 4] = std::array::from_fn(|c| {
                if c == 3 {
                    return (brightness * 255.).round() as u8;
                }
                let colour = p[c + 1].into() as f32 / (p[0].into() as f32 * 63.);
                (255. * if transparent { 1. } else { brightness } * colour.clamp(0., 1.).sqrt())
                    as u8
            });
            rgb.into_iter().take(if transparent { 4 } else { 3 })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_variation_subsets_are_respected() {
        let masks = (1u64..32).chain((0..crate::settings::VARIATION_COUNT).map(|i| 1u64 << i));
        for mask in masks {
            let options = crate::settings::GenerationOptions {
                enabled: std::array::from_fn(|i| mask & (1 << i) != 0),
            };
            for seed in 1..=4 {
                let f = generate_with(seed, &options);
                for t in &f.transforms {
                    assert!(options.enabled[t.variation[0] as usize]);
                    if !options.enabled[0] {
                        assert_eq!(t.variation[1], 1.);
                    }
                }
            }
        }
    }
    #[test]
    fn variation_reference_values_and_origin_are_finite() {
        let mut t = generate(1).transforms[0];
        t.row0 = [1., 0., 0., 1.];
        t.row1 = [0., 1., 0., 1.];
        for kind in 0..crate::settings::VARIATION_COUNT {
            t.variation = [kind as f32, 1., 0., 0.];
            assert!(apply(&t, [0., 0.]).iter().all(|v| v.is_finite()));
        }
        for (kind, input, expected) in [
            (5, [1., 0.], [0.5, 0.]),
            (13, [-2., -3.], [-4., -1.5]),
            (14, [1., 0.], [0., 1.]),
            (15, [1., 0.], [1., 0.]),
            (16, [1., 0.], [0.8, 0.]),
        ] {
            t.variation[0] = kind as f32;
            let actual = apply(&t, input);
            assert!(
                (actual[0] - expected[0]).abs() < 1e-5 && (actual[1] - expected[1]).abs() < 1e-5
            );
        }
    }
    #[test]
    fn generation_is_reproducible_and_varied() {
        let a = generate(123);
        let b = generate(123);
        let c = generate(456);
        assert_eq!(a.transforms, b.transforms);
        assert_eq!(a.camera, b.camera);
        assert_ne!(a.transforms, c.transforms);
    }
    #[test]
    fn batch_has_valid_probabilities_and_bounds() {
        for seed in 1..49 {
            let f = generate(seed);
            assert!(f.camera.iter().all(|v| v.is_finite()));
            assert!(f.camera[2] > 0.);
            assert_eq!(f.transforms[3].row1[3], 1.);
            assert!(f.transforms.windows(2).all(|t| t[0].row1[3] < t[1].row1[3]));
            assert!(f.transforms.iter().any(|t| t.variation[0] > 0.));
        }
    }
    #[test]
    fn empty_histogram_is_background() {
        assert_eq!(colours(&[[0u32; 4]], 1.), vec![5, 7, 12]);
    }
}
