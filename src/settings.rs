pub const VARIATION_COUNT: usize = 40;
pub const VARIATIONS: [(&str, &str); VARIATION_COUNT] = [
    ("Linear", "Straight affine transformations"),
    ("Sinusoidal", "Waves and bounded folds"),
    ("Spherical", "Inversion around the origin"),
    ("Swirl", "Twisting curves"),
    ("Horseshoe", "Folded and stretched shapes"),
    ("Polar", "Unwrap angle and radius"),
    ("Handkerchief", "Folded radial waves"),
    ("Heart", "Heart-shaped radial folds"),
    ("Disc", "Map angle onto a rippled disc"),
    ("Spiral", "Reciprocal radial spirals"),
    ("Hyperbolic", "Stretch one axis and invert the other"),
    ("Diamond", "Angular diamond folds"),
    ("Ex", "Cubic radial lobes"),
    ("Bent", "Bend the negative coordinate axes"),
    ("Fisheye", "Swap axes with radial compression"),
    ("Eyefish", "Radial compression without swapping axes"),
    ("Bubble", "Smooth spherical compression"),
    ("Cylinder", "Wrap the horizontal coordinate"),
    ("Julia", "Random square-root branches"),
    ("Waves", "Parameterized sinusoidal displacement"),
    ("Popcorn", "Trigonometric displacement"),
    ("Exponential", "Exponential polar mapping"),
    ("Power", "Angle-dependent radial power"),
    ("Cosine", "Complex cosine mapping"),
    ("Rings", "Concentric affine rings"),
    ("Fan", "Angular affine sectors"),
    ("Noise", "Random coordinate modulation"),
    ("Blur", "Uniform radial blur"),
    ("Gaussian blur", "Gaussian-like radial blur"),
    ("Pre blur", "Jitter before the affine transformation"),
    ("Radial blur", "Random spin and zoom"),
    ("Rings2", "Parameterized radial folds"),
    ("Fan2", "Parameterized angular folds"),
    ("Blob", "Lobed radial modulation"),
    ("PDJ", "Four-parameter trigonometric attractor"),
    ("Perspective", "Perspective projection"),
    ("Julian", "Random generalized Julia branches"),
    ("Juliascope", "Reflected Julia branches"),
    ("Curl", "Complex rational curl"),
    ("Rectangles", "Rectangular coordinate folds"),
];
#[derive(Clone, Copy)]
pub struct GenerationOptions {
    pub enabled: [bool; VARIATION_COUNT],
}
impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            enabled: [true; VARIATION_COUNT],
        }
    }
}
impl GenerationOptions {
    pub fn valid(&self) -> bool {
        self.enabled.iter().any(|&v| v)
    }
    pub fn load() -> Self {
        let Some(path) = settings_path() else {
            return Self::default();
        };
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| Self::parse(s.trim()))
            .unwrap_or_default()
    }
    fn parse(s: &str) -> Option<Self> {
        if (s.len() != 5 && s.len() != 18 && s.len() != VARIATION_COUNT)
            || !s.bytes().all(|b| b == b'0' || b == b'1')
        {
            return None;
        }
        let options = Self {
            // Preserve existing choices; newly added variations start disabled on migration.
            enabled: std::array::from_fn(|i| s.as_bytes().get(i) == Some(&b'1')),
        };
        options.valid().then_some(options)
    }
    pub fn save(&self) -> Result<(), String> {
        let path = settings_path().ok_or("Could not find the local settings folder")?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let text: String = self
            .enabled
            .iter()
            .map(|&b| if b { '1' } else { '0' })
            .collect();
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}
fn settings_path() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|p| {
        std::path::PathBuf::from(p)
            .join("Bapophysis")
            .join("variations.txt")
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_preferences_fall_back_safely() {
        for text in ["", "00000", "111", "abcde", "111111"] {
            assert!(GenerationOptions::parse(text).is_none());
        }
        assert_eq!(
            GenerationOptions::parse("10010").unwrap().enabled[..5],
            [true, false, false, true, false]
        );
    }
}
#[derive(Clone, Copy)]
pub struct ExportOptions {
    pub transparent: bool,
    pub resolution: u32,
    pub width: u32,
    pub height: u32,
}
impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            transparent: false,
            resolution: 0,
            width: 1920,
            height: 1080,
        }
    }
}
impl ExportOptions {
    pub fn dimensions(&self, preview: [u32; 2]) -> [u32; 2] {
        match self.resolution {
            0 => preview,
            1 => [self.width.clamp(64, 2048), self.height.clamp(64, 2048)],
            edge => {
                let scale = edge.min(2048) as f32 / preview[0].max(preview[1]).max(1) as f32;
                [
                    (preview[0] as f32 * scale).round().max(1.) as u32,
                    (preview[1] as f32 * scale).round().max(1.) as u32,
                ]
            }
        }
    }
    pub fn load() -> Self {
        let Some(path) = settings_path().map(|p| p.with_file_name("export.txt")) else {
            return Self::default();
        };
        let Some(text) = std::fs::read_to_string(path).ok() else {
            return Self::default();
        };
        let values: Vec<u32> = text
            .split_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
        if values.len() != 4 || values[0] > 1 || ![0, 1, 1024, 1536, 2048].contains(&values[1]) {
            return Self::default();
        }
        Self {
            transparent: values[0] == 1,
            resolution: values[1],
            width: values[2].clamp(64, 2048),
            height: values[3].clamp(64, 2048),
        }
    }
    pub fn save(&self) -> Result<(), String> {
        let path = settings_path()
            .ok_or("Could not find settings folder")?
            .with_file_name("export.txt");
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(
            path,
            format!(
                "{} {} {} {}",
                u32::from(self.transparent),
                self.resolution,
                self.width,
                self.height
            ),
        )
        .map_err(|e| e.to_string())
    }
}
