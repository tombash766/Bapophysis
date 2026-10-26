#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod flame;
mod gpu;
use eframe::egui;
use flame::{colours, generate, Flame, Random};
use gpu::{Gpu, PARTICLES};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SIZE: u32 = 768;
const THUMB: u32 = 160;
// Bounds fixed-point colour accumulation even if every point hits one pixel.
const SAMPLE_LIMIT: u64 = 32_000_000;
struct Preview {
    flame: Flame,
    texture: Option<egui::TextureHandle>,
}
struct App {
    detail: Gpu,
    thumbnails: Gpu,
    adapter: String,
    previews: Vec<Preview>,
    selected: usize,
    batch_size: usize,
    batch_id: u64,
    thumbnail_job: Option<(u64, usize)>,
    rng: Random,
    camera: [f32; 4],
    dirty: bool,
    running: bool,
    samples: u64,
    pixels: Vec<[u32; 4]>,
    texture: Option<egui::TextureHandle>,
    exposure: f32,
    export_path: String,
    status: String,
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let render = cc
            .wgpu_render_state
            .as_ref()
            .expect("GPU renderer unavailable");
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u32;
        let mut app = Self {
            detail: Gpu::new(render.device.clone(), render.queue.clone(), SIZE, SIZE),
            thumbnails: Gpu::new(render.device.clone(), render.queue.clone(), THUMB, THUMB),
            adapter: render.adapter.get_info().name,
            previews: Vec::new(),
            selected: 0,
            batch_size: 24,
            batch_id: 0,
            thumbnail_job: None,
            rng: Random(seed),
            camera: [0., 0., 5., 5.],
            dirty: true,
            running: true,
            samples: 0,
            pixels: vec![[0; 4]; (SIZE * SIZE) as usize],
            texture: None,
            exposure: 1.,
            export_path: String::new(),
            status: "Choose a preview to explore it".into(),
        };
        app.new_batch();
        app
    }
    fn new_batch(&mut self) {
        self.batch_id += 1;
        self.previews = (0..self.batch_size)
            .map(|_| Preview {
                flame: generate(self.rng.next()),
                texture: None,
            })
            .collect();
        self.select(0);
        self.status = format!("Generating {} previews…", self.batch_size);
    }
    fn select(&mut self, index: usize) {
        self.selected = index;
        self.camera = self.previews[index].flame.camera;
        self.texture = self.previews[index].texture.clone();
        self.dirty = true;
        self.running = true;
        self.samples = 0;
        self.pixels.fill([0; 4]);
        self.export_path = format!("exports/flame-{}.png", self.previews[index].flame.seed);
    }
    fn update_texture(&mut self, ctx: &egui::Context) {
        let image = egui::ColorImage::from_rgb(
            [SIZE as usize, SIZE as usize],
            &colours(&self.pixels, self.exposure),
        );
        // A selected thumbnail handle must never be edited in place.
        if let Some(texture) = self
            .texture
            .as_mut()
            .filter(|t| t.size() == [SIZE as usize, SIZE as usize])
        {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            self.texture =
                Some(ctx.load_texture("Selected flame", image, egui::TextureOptions::LINEAR));
        }
    }
    fn collect(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.thumbnails.collect() {
            let job = self.thumbnail_job.take();
            match result {
                Ok(pixels) => {
                    if let Some((batch, index)) = job {
                        if batch == self.batch_id {
                            let image = egui::ColorImage::from_rgb(
                                [THUMB as usize, THUMB as usize],
                                &colours(&pixels, 1.),
                            );
                            let texture = ctx.load_texture(
                                format!("Preview {batch}/{index}"),
                                image,
                                egui::TextureOptions::LINEAR,
                            );
                            if index == self.selected && self.samples == 0 {
                                self.texture = Some(texture.clone());
                            }
                            self.previews[index].texture = Some(texture);
                        }
                    }
                }
                Err(e) => self.status = format!("Preview failed: {e}"),
            }
        }
        if let Some(result) = self.detail.collect() {
            match result {
                Ok(pixels) if !self.dirty => {
                    self.pixels = pixels;
                    self.samples += (PARTICLES * 32) as u64;
                    self.update_texture(ctx);
                }
                Ok(_) => (),
                Err(e) => {
                    self.running = false;
                    self.status = format!("Render failed: {e}");
                }
            }
        }
    }
    fn schedule(&mut self) {
        if self.thumbnails.pending.is_none() {
            if let Some(index) = self.previews.iter().position(|p| p.texture.is_none()) {
                self.thumbnails
                    .reset(&self.previews[index].flame.params(THUMB, THUMB));
                self.thumbnail_job = Some((self.batch_id, index));
                self.thumbnails.dispatch();
            }
        }
        if self.detail.pending.is_none() {
            if self.dirty {
                let mut p = self.previews[self.selected].flame.params(SIZE, SIZE);
                p.camera = self.camera;
                self.detail.reset(&p);
                self.dirty = false;
            }
            if self.running && self.samples < SAMPLE_LIMIT {
                self.detail.dispatch();
            } else if self.samples >= SAMPLE_LIMIT {
                self.running = false;
            }
        }
    }
    fn export(&mut self) {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let path = std::path::Path::new(&self.export_path);
            if path.exists() {
                return Err("File exists; choose a new filename".into());
            }
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            let mut enc = png::Encoder::new(std::io::BufWriter::new(file), SIZE, SIZE);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            enc.add_text_chunk(
                "Flame seed".into(),
                self.previews[self.selected].flame.seed.to_string(),
            )?;
            enc.write_header()?
                .write_image_data(&colours(&self.pixels, self.exposure))?;
            Ok(())
        })();
        self.status = match result {
            Ok(()) => format!("Saved {}", self.export_path),
            Err(e) => format!("Export failed: {e}"),
        };
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.collect(ctx);
        egui::SidePanel::left("gallery")
            .exact_width(270.)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(8.);
                ui.heading("Bapophysis");
                ui.label("Random flame explorer");
                ui.add_space(8.);
                ui.horizontal(|ui| {
                    if ui.button("New batch").clicked() {
                        self.new_batch();
                    }
                    egui::ComboBox::from_id_salt("batch size")
                        .selected_text(format!("{} flames", self.batch_size))
                        .show_ui(ui, |ui| {
                            for size in [12, 24, 48] {
                                ui.selectable_value(
                                    &mut self.batch_size,
                                    size,
                                    format!("{size} flames"),
                                );
                            }
                        });
                });
                let ready = self.previews.iter().filter(|p| p.texture.is_some()).count();
                ui.small(format!(
                    "{ready}/{} previews · click to refine",
                    self.previews.len()
                ));
                ui.separator();
                let mut selection = None;
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("preview grid")
                            .num_columns(2)
                            .spacing([6., 8.])
                            .show(ui, |ui| {
                                for (index, p) in self.previews.iter().enumerate() {
                                    ui.vertical(|ui| {
                                        let selected = index == self.selected;
                                        let response = if let Some(texture) = &p.texture {
                                            ui.add(
                                                egui::ImageButton::new(
                                                    egui::Image::new(texture)
                                                        .fit_to_exact_size(egui::vec2(115., 115.)),
                                                )
                                                .selected(selected),
                                            )
                                        } else {
                                            ui.add_sized(
                                                [123., 123.],
                                                egui::Button::new(format!(
                                                    "{}\nPreparing…",
                                                    index + 1
                                                ))
                                                .selected(selected),
                                            )
                                        };
                                        if response.clicked() {
                                            selection = Some(index);
                                        }
                                        ui.small(format!(
                                            "#{:02} · {:08x}",
                                            index + 1,
                                            p.flame.seed
                                        ));
                                    });
                                    if index % 2 == 1 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
                if let Some(index) = selection {
                    self.select(index);
                }
            });
        egui::TopBottomPanel::top("render controls").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("Flame #{:02}", self.selected + 1));
                if ui
                    .button(if self.running { "Pause" } else { "Refine" })
                    .clicked()
                {
                    if self.samples >= SAMPLE_LIMIT {
                        self.dirty = true;
                        self.samples = 0;
                    }
                    self.running = !self.running;
                }
                if ui.button("Fit view").clicked() {
                    self.camera = self.previews[self.selected].flame.camera;
                    self.samples = 0;
                    self.dirty = true;
                    self.running = true;
                }
                ui.label(format!("{:.1}M iterations", self.samples as f64 / 1e6));
                if ui
                    .add(egui::Slider::new(&mut self.exposure, 0.3..=2.5).text("Exposure"))
                    .changed()
                    && self.samples > 0
                {
                    self.update_texture(ctx);
                }
            });
        });
        egui::TopBottomPanel::bottom("export").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("PNG");
                ui.add(egui::TextEdit::singleline(&mut self.export_path).desired_width(230.));
                if ui
                    .add_enabled(
                        self.samples > 0 && !self.dirty,
                        egui::Button::new("Save image"),
                    )
                    .clicked()
                {
                    self.export();
                }
            });
            let ready = self.previews.iter().filter(|p| p.texture.is_some()).count();
            if ready == self.previews.len() && self.status.starts_with("Generating") {
                self.status = "Batch ready. Choose a preview, or generate more.".into();
            }
            ui.small(&self.status);
            ui.small(format!("{} · Drag to pan · Scroll to zoom", self.adapter));
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(texture) = &self.texture {
                let side = ui.available_width().min(ui.available_height()).max(1.);
                let response = ui.add(
                    egui::Image::new(texture)
                        .fit_to_exact_size(egui::vec2(side, side))
                        .sense(egui::Sense::drag()),
                );
                if response.dragged() {
                    let delta = ctx.input(|i| i.pointer.delta());
                    self.camera[0] -= delta.x / side * self.camera[2];
                    self.camera[1] += delta.y / side * self.camera[3];
                    self.dirty = true;
                    self.samples = 0;
                    self.running = true;
                }
                if response.hovered() {
                    let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0. {
                        let extent = (self.camera[2] * (-scroll * 0.002).exp()).clamp(0.001, 1000.);
                        self.camera[2] = extent;
                        self.camera[3] = extent;
                        self.dirty = true;
                        self.samples = 0;
                        self.running = true;
                    }
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            }
        });
        self.schedule();
        if self.running
            || self.detail.pending.is_some()
            || self.thumbnails.pending.is_some()
            || self.dirty
        {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}
fn main() -> eframe::Result {
    if std::env::args().any(|a| a == "--gpu-check") {
        gpu_check();
        return Ok(());
    }
    eframe::run_native(
        "Bapophysis",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
                wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
                    eframe::egui_wgpu::WgpuSetupCreateNew {
                        instance_descriptor: wgpu::InstanceDescriptor {
                            backends: wgpu::Backends::DX12,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
                ..Default::default()
            },
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1120., 900.])
                .with_min_inner_size([800., 600.]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
fn await_pixels(gpu: &mut Gpu) -> Vec<[u32; 4]> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(result) = gpu.collect() {
            return result.expect("GPU readback failed");
        }
        assert!(Instant::now() < deadline, "GPU timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn write_png(path: &str, size: u32, pixels: &[[u32; 4]]) {
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), size, size);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&colours(pixels, 1.))
        .unwrap();
}
fn gpu_check() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .expect("No compatible GPU");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .unwrap();
    let mut gpu = Gpu::new(device.clone(), queue.clone(), THUMB, THUMB);
    std::fs::create_dir_all("exports").unwrap();
    let mut first = None;
    for seed in 1..=12 {
        let flame = generate(seed);
        gpu.reset(&flame.params(THUMB, THUMB));
        gpu.dispatch();
        let pixels = await_pixels(&mut gpu);
        let hits: u64 = pixels.iter().map(|p| p[0] as u64).sum();
        let occupied = pixels.iter().filter(|p| p[0] > 0).count();
        assert!(
            hits > 50_000 && occupied > 150,
            "Blank flame seed {seed}: {hits} hits, {occupied} pixels"
        );
        assert!(pixels
            .iter()
            .all(|p| p[1..].iter().all(|&c| c as u64 <= p[0] as u64 * 63)));
        write_png(&format!("exports/check-flame-{seed}.png"), THUMB, &pixels);
        if seed == 1 {
            first = Some(pixels);
        }
    }
    let flame = generate(1);
    gpu.reset(&flame.params(THUMB, THUMB));
    gpu.dispatch();
    assert_eq!(first.unwrap(), await_pixels(&mut gpu));
    let mut detail = Gpu::new(device, queue, SIZE, SIZE);
    detail.reset(&flame.params(SIZE, SIZE));
    let mut final_pixels = Vec::new();
    for _ in 0..16 {
        detail.dispatch();
        final_pixels = await_pixels(&mut detail);
    }
    write_png("exports/check-detail.png", SIZE, &final_pixels);
    println!(
        "GPU check passed: {} | 12 coloured flame previews, deterministic reset, detailed render",
        adapter.get_info().name
    );
}
