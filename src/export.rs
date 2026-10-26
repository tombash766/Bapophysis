use std::{io::Write, path::Path};
pub fn save_png<T: Copy + Into<u64>>(
    path: &Path,
    width: u32,
    height: u32,
    pixels: &[[T; 4]],
    exposure: f32,
    seed: u32,
    transparent: bool,
) -> Result<(), String> {
    if width == 0 || height == 0 || pixels.len() != (width as usize) * (height as usize) {
        return Err("Image is not ready to save".into());
    }
    // Finish encoding before creating a file, and propagate final write errors.
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(if transparent {
            png::ColorType::Rgba
        } else {
            png::ColorType::Rgb
        });
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .add_text_chunk("Flame seed".into(), seed.to_string())
            .map_err(|e| e.to_string())?;
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(&crate::flame::image_bytes(pixels, exposure, transparent))
            .map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
    }
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(&bytes)?;
        file.flush()
    })();
    result.map_err(|e| format!("Could not save {}: {e}", path.display()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exported_png_decodes_and_existing_files_are_protected() {
        let path =
            std::env::temp_dir().join(format!("bapophysis-export-test-{}.png", std::process::id()));
        let pixels = vec![[1u32, 63, 20, 10]; 6];
        save_png(&path, 3, 2, &pixels, 1., 123, false).unwrap();
        let data = std::fs::read(&path).unwrap();
        let mut reader = png::Decoder::new(data.as_slice()).read_info().unwrap();
        assert_eq!((reader.info().width, reader.info().height), (3, 2));
        assert_eq!(reader.info().uncompressed_latin1_text[0].text, "123");
        let mut decoded = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut decoded).unwrap();
        assert_eq!(decoded, crate::flame::colours(&pixels, 1.));
        assert!(save_png(&path, 3, 2, &pixels, 1., 123, false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), data);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn write_failures_include_destination() {
        let path = std::env::temp_dir()
            .join("missing-bapophysis-folder")
            .join("test.png");
        let error = save_png(&path, 1, 1, &[[0u32; 4]], 1., 1, false).unwrap_err();
        assert!(error.contains("test.png"));
    }
    #[test]
    fn transparent_png_has_straight_alpha_and_empty_background() {
        let path =
            std::env::temp_dir().join(format!("bapophysis-alpha-test-{}.png", std::process::id()));
        let pixels = [[0u32; 4], [1, 63, 20, 10], [100, 6300, 2000, 1000]];
        save_png(&path, 3, 1, &pixels, 1., 123, true).unwrap();
        let data = std::fs::read(&path).unwrap();
        let mut reader = png::Decoder::new(data.as_slice()).read_info().unwrap();
        assert_eq!(reader.info().color_type, png::ColorType::Rgba);
        let mut bytes = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut bytes).unwrap();
        assert_eq!(&bytes[..4], &[0, 0, 0, 0]);
        assert!(bytes[7] > 0 && bytes[7] < 255);
        assert_eq!(bytes[11], 255);
        let opaque = crate::flame::colours(&pixels, 1.);
        for i in 1..3 {
            for c in 0..3 {
                assert!(
                    (bytes[i * 4 + c] as f32 * bytes[i * 4 + 3] as f32 / 255.
                        - opaque[i * 3 + c] as f32)
                        .abs()
                        < 2.
                );
            }
        }
        std::fs::remove_file(path).unwrap();
    }
}
