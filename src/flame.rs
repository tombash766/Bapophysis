use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Transform {
    pub row0: [f32; 4],
    pub row1: [f32; 4],
    pub variation: [f32; 4], // kind, nonlinear blend, colour coordinate, unused
}
#[derive(Clone)]
pub struct Flame {
    pub seed: u32,
    pub transforms: [Transform; 4],
    pub camera: [f32; 4],
    pub palette: [[f32; 4]; 3],
}
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Params {
    pub camera: [f32; 4],
    pub size: [u32; 4],
    pub palette: [[f32; 4]; 3],
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
    let x = t.row0[0] * p[0] + t.row0[1] * p[1] + t.row0[2];
    let y = t.row1[0] * p[0] + t.row1[1] * p[1] + t.row1[2];
    let r2 = (x * x + y * y).max(0.03);
    let r = r2.sqrt();
    let v = match t.variation[0] as u32 {
        1 => [x.sin(), y.sin()],
        2 => [x / r2, y / r2],
        3 => [x * r2.sin() - y * r2.cos(), x * r2.cos() + y * r2.sin()],
        4 => [(x - y) * (x + y) / r, 2. * x * y / r],
        _ => [x, y],
    };
    let blend = t.variation[1];
    [x + (v[0] - x) * blend, y + (v[1] - y) * blend]
}
fn palette(hue: f32) -> [[f32; 4]; 3] {
    std::array::from_fn(|i| {
        let h = hue + i as f32 * 0.19;
        let rgb: [f32; 3] = std::array::from_fn(|c| {
            0.2 + 0.8 * (0.5 + 0.5 * (std::f32::consts::TAU * (h + c as f32 / 3.)).cos())
        });
        [rgb[0], rgb[1], rgb[2], 1.]
    })
}
fn candidate(seed: u32) -> Flame {
    let mut rng = Random(seed);
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
                if i == 0 {
                    1. + (rng.next() % 4) as f32
                } else {
                    (rng.next() % 5) as f32
                },
                rng.range(0.45, 1.),
                rng.unit(),
                0.,
            ],
        }
    });
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
        palette: palette(rng.unit()),
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
        p = apply(t, p);
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
    let mut rng = Random(seed);
    for _ in 0..64 {
        let mut flame = candidate(rng.next());
        if fit(&mut flame) {
            return flame;
        }
    }
    // Deterministic known-good search avoids returning a blank candidate.
    let mut flame = candidate(13579);
    if !fit(&mut flame) {
        flame.camera = [0., 0., 5., 5.];
    }
    flame
}
pub fn colours(pixels: &[[u32; 4]], exposure: f32) -> Vec<u8> {
    let mut densities: Vec<u32> = pixels.iter().map(|p| p[0]).filter(|&n| n > 0).collect();
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
            if p[0] == 0 {
                return [5, 7, 12];
            }
            let brightness = ((p[0] as f32).ln_1p() / reference.ln_1p() * exposure)
                .clamp(0., 1.)
                .powf(0.65);
            std::array::from_fn::<_, 3, _>(|c| {
                let colour = p[c + 1] as f32 / (p[0] as f32 * 63.);
                (255. * brightness * colour.clamp(0., 1.).sqrt()) as u8
            })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(colours(&[[0; 4]], 1.), vec![5, 7, 12]);
    }
}
