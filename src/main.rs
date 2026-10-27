#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod export;
mod flame;
mod gpu;
mod gradients;
mod settings;
use eframe::egui;
use flame::{colours, generate, generate_with, Flame, Random};
use gpu::{Gpu, PARTICLES};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SIZE: u32 = 768;
const THUMB: u32 = 160;
struct Preview {
    flame: Flame,
    texture: Option<egui::TextureHandle>,
    gradient: usize,
    original_palette: gradients::Palette,
    base_palette: gradients::Palette,
    rotation: usize,
}
const EXPORT_BATCHES: u32 = 8;
struct ExportJob {
    gpu: Gpu,
    path: std::path::PathBuf,
    size: [u32; 2],
    samples: u64,
    target: u64,
    exposure: f32,
    seed: u32,
    transparent: bool,
    pixels: Vec<[u64; 4]>,
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
    render_camera: [f32; 4],
    texture_camera: [f32; 4],
    dirty: bool,
    running: bool,
    samples: u64,
    pixels: Vec<[u64; 4]>,
    preview_batches: u32,
    pending_preview_batches: u32,
    iteration_target: u64,
    texture: Option<egui::TextureHandle>,
    exposure: f32,
    render_size: [u32; 2],
    gpu_size: [u32; 2],
    preferences_open: bool,
    gradient_open: bool,
    gradient_batch: u32,
    gradient_options: gradients::RandomOptions,
    options: settings::GenerationOptions,
    export_options: settings::ExportOptions,
    export_job: Option<ExportJob>,
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
            render_camera: [0., 0., 5., 5.],
            texture_camera: [0., 0., 5., 5.],
            dirty: true,
            running: true,
            samples: 0,
            pixels: vec![[0; 4]; (SIZE * SIZE) as usize],
            preview_batches: settings::load_render_batches(),
            pending_preview_batches: 1,
            iteration_target: settings::load_iteration_target(),
            texture: None,
            exposure: 1.,
            render_size: [SIZE, SIZE],
            gpu_size: [SIZE, SIZE],
            preferences_open: false,
            gradient_open: false,
            gradient_batch: seed,
            gradient_options: gradients::RandomOptions::default(),
            options: settings::GenerationOptions::load(),
            export_options: settings::ExportOptions::load(),
            export_job: None,
            status: "Choose a preview to explore it".into(),
        };
        app.new_batch();
        app
    }
    fn new_batch(&mut self) {
        self.batch_id += 1;
        self.previews = (0..self.batch_size)
            .map(|_| {
                let flame = generate_with(self.rng.next(), &self.options);
                Preview {
                    original_palette: flame.palette,
                    base_palette: flame.palette,
                    rotation: 0,
                    flame,
                    texture: None,
                    gradient: 0,
                }
            })
            .collect();
        self.select(0);
        self.status = format!("Generating {} previews…", self.batch_size);
    }
    fn select(&mut self, index: usize) {
        self.selected = index;
        self.camera = self.previews[index].flame.camera;
        self.camera[2] = self.camera[3] * self.render_size[0] as f32 / self.render_size[1] as f32;
        self.texture = self.previews[index].texture.clone();
        self.texture_camera = self.camera;
        self.dirty = true;
        self.running = true;
        self.samples = 0;
        self.pixels.fill([0; 4]);
    }
    fn update_texture(&mut self, ctx: &egui::Context) {
        self.texture_camera = self.render_camera;
        let image = egui::ColorImage::from_rgb(
            [self.render_size[0] as usize, self.render_size[1] as usize],
            &colours(&self.pixels, self.exposure),
        );
        // A selected thumbnail handle must never be edited in place.
        if let Some(texture) = self
            .texture
            .as_mut()
            .filter(|t| t.size() == [self.render_size[0] as usize, self.render_size[1] as usize])
        {
            texture.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture =
                Some(ctx.load_texture("Selected flame", image, egui::TextureOptions::NEAREST));
        }
    }
    fn change_gradient(&mut self, id: usize) {
        let p = &mut self.previews[self.selected];
        p.gradient = id;
        p.base_palette = if id == 0 {
            p.original_palette
        } else {
            gradients::palette(id)
        };
        p.rotation = 0;
        p.flame.palette = p.base_palette;
        self.refresh_gradient();
    }
    fn refresh_gradient(&mut self) {
        let p = &mut self.previews[self.selected];
        p.flame.palette = gradients::rotated(&p.base_palette, p.rotation);
        p.texture = None;
        // Invalidate any pending thumbnail without replacing the rest of the batch.
        self.batch_id += 1;
        self.dirty = true;
        self.samples = 0;
        self.running = true;
        self.texture = None;
    }
    fn random_gradient(&mut self, seed: u32) {
        let p = &mut self.previews[self.selected];
        p.gradient = gradients::NAMES.len();
        p.base_palette = gradients::random_with(seed, &self.gradient_options);
        p.rotation = 0;
        self.refresh_gradient();
    }
    fn gradient_popup(&mut self, ctx: &egui::Context) {
        if !self.gradient_open {
            return;
        }
        let mut open = self.gradient_open;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("gradient window"),
            egui::ViewportBuilder::default()
                .with_title("Bapophysis — Gradient")
                .with_inner_size([480., 680.])
                .with_min_inner_size([460., 400.])
                .with_window_level(egui::WindowLevel::Normal),
            |ctx, _| {
                if ctx.input(|i| i.viewport().close_requested()) {
                    open = false;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().id_salt("gradient content").show(ui, |ui| {
                ui.label(format!("Colours for fractal #{:02}", self.selected + 1));
                ui.small("Changes apply only to this fractal and its thumbnail.");
                if let Some(texture) = &self.texture {
                    let size = texture.size_vec2();
                    let scale = (140. / size.x).min(120. / size.y);
                    ui.add(egui::Image::new(texture).fit_to_exact_size(size * scale));
                }
                gradient_strip(
                    ui,
                    &self.previews[self.selected].flame.palette,
                    egui::vec2(420., 32.),
                );
                let p = &mut self.previews[self.selected];
                if ui
                    .add(egui::Slider::new(&mut p.rotation, 0..=255).text("Rotate"))
                    .changed()
                {
                    self.refresh_gradient();
                }
                ui.separator();
                ui.label("Random gradient settings");
                ui.add(egui::Slider::new(&mut self.gradient_options.hue_range, 0.0..=360.0).text("Hue range (degrees)"));
                ui.add_enabled(self.gradient_options.hue_range < 360., egui::Slider::new(&mut self.gradient_options.hue, 0.0..=360.0).text("Centre hue (degrees)"));
                ui.add(egui::Slider::new(&mut self.gradient_options.saturation, 0.0..=1.0).text("Saturation"));
                ui.small("0° range = one hue; 360° = all hues. Settings affect new random gradients and the choices below.");
                ui.horizontal(|ui| {
                    if ui.button("New random gradient").clicked() {
                        let seed = self.rng.next();
                        self.random_gradient(seed);
                    }
                    if ui.button("Shuffle choices").clicked() {
                        self.gradient_batch = self.rng.next();
                    }
                    if ui.button("Restore original").clicked() {
                        self.change_gradient(0);
                    }
                });
                let mut chosen = self.previews[self.selected].gradient;
                egui::ComboBox::from_id_salt("gradient presets")
                    .selected_text(
                        gradients::NAMES
                            .get(chosen)
                            .copied()
                            .unwrap_or("Random gradient"),
                    )
                    .show_ui(ui, |ui| {
                        for (id, name) in gradients::NAMES.iter().enumerate() {
                            ui.selectable_value(&mut chosen, id, *name);
                        }
                    });
                if chosen != self.previews[self.selected].gradient {
                    self.change_gradient(chosen);
                }
                ui.separator();
                ui.small("Random palettes · click a strip to apply");
                egui::ScrollArea::vertical()
                    .id_salt("random palettes")
                    .max_height(180.)
                    .show(ui, |ui| {
                        for i in 0..10u32 {
                            let seed = self.gradient_batch.wrapping_add(i.wrapping_mul(747796405));
                            let palette = gradients::random_with(seed, &self.gradient_options);
                            if gradient_strip(ui, &palette, egui::vec2(420., 22.)).clicked() {
                                self.random_gradient(seed);
                            }
                            ui.add_space(3.);
                        }
                    });
                });
                });
            },
        );
        self.gradient_open = open;
    }
    fn collect(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.export_job.as_mut().and_then(|job| job.gpu.collect()) {
            match result {
                Ok(pixels) => {
                    let job = self.export_job.as_mut().unwrap();
                    accumulate(&mut job.pixels, &pixels);
                    job.gpu.clear_histogram();
                    job.samples += (PARTICLES * 32 * EXPORT_BATCHES) as u64;
                    if job.samples >= job.target {
                        let job = self.export_job.take().unwrap();
                        self.status = match export::save_png(
                            &job.path,
                            job.size[0],
                            job.size[1],
                            &job.pixels,
                            job.exposure,
                            job.seed,
                            job.transparent,
                        ) {
                            Ok(()) => format!(
                                "Saved {} ({} × {})",
                                job.path.display(),
                                job.size[0],
                                job.size[1]
                            ),
                            Err(e) => e,
                        };
                    }
                }
                Err(e) => {
                    self.export_job = None;
                    self.status = format!("Export render failed: {e}");
                }
            }
        }
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
                                self.texture_camera = self.previews[index].flame.camera;
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
                    accumulate(&mut self.pixels, &pixels);
                    self.detail.clear_histogram();
                    self.samples += (PARTICLES * 32 * self.pending_preview_batches) as u64;
                    if self.samples >= self.iteration_target {
                        self.running = false;
                    }
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
        if let Some(job) = &mut self.export_job {
            if job.gpu.pending.is_none() {
                job.gpu.dispatch_batches(EXPORT_BATCHES);
            }
        }
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
                if self.gpu_size != self.render_size {
                    self.detail = self
                        .detail
                        .resized(self.render_size[0], self.render_size[1]);
                    self.gpu_size = self.render_size;
                }
                let mut p = self.previews[self.selected]
                    .flame
                    .params(self.render_size[0], self.render_size[1]);
                p.camera = self.camera;
                self.detail.reset(&p);
                self.render_camera = self.camera;
                self.pixels = vec![[0; 4]; (self.render_size[0] * self.render_size[1]) as usize];
                self.dirty = false;
            }
            if self.running && self.samples < self.iteration_target {
                self.pending_preview_batches = self.preview_batches;
                self.detail.dispatch_batches(self.pending_preview_batches);
            } else if self.samples >= self.iteration_target {
                self.running = false;
            }
        }
    }
    fn export(&mut self) {
        let name = format!("flame-{}.png", self.previews[self.selected].flame.seed);
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save fractal flame")
            .add_filter("PNG image", &["png"])
            .set_file_name(&name);
        if let Some(home) = std::env::var_os("USERPROFILE") {
            let home = std::path::PathBuf::from(home);
            let pictures = home.join("Pictures");
            dialog = dialog.set_directory(if pictures.is_dir() { pictures } else { home });
        }
        let Some(mut path) = dialog.save_file() else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("png");
        }
        let size = self.export_options.dimensions(self.render_size);
        let seed = self.previews[self.selected].flame.seed;
        if path.exists() {
            self.status = "Please choose a new filename; existing files are protected.".into();
            return;
        }
        if size == self.render_size {
            self.status = match export::save_png(
                &path,
                size[0],
                size[1],
                &self.pixels,
                self.exposure,
                seed,
                self.export_options.transparent,
            ) {
                Ok(()) => format!("Saved {}", path.display()),
                Err(e) => e,
            };
        } else {
            let mut gpu = self.detail.resized(size[0], size[1]);
            let mut params = self.previews[self.selected].flame.params(size[0], size[1]);
            params.camera = export_camera(self.camera, size);
            gpu.reset(&params);
            self.export_job = Some(ExportJob {
                gpu,
                path,
                size,
                samples: 0,
                target: ((size[0] as u64 * size[1] as u64) * 32)
                    .clamp(8_000_000, 64_000_000)
                    .max(self.iteration_target),
                exposure: self.exposure,
                seed,
                transparent: self.export_options.transparent,
                pixels: vec![[0; 4]; (size[0] * size[1]) as usize],
            });
            self.status = "Rendering PNG at the selected resolution…".into();
        }
    }
    fn preferences(&mut self, ctx: &egui::Context) {
        if !self.preferences_open {
            return;
        }
        let mut open = self.preferences_open;
        let mut generate = false;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("preferences window"),
            egui::ViewportBuilder::default()
                .with_title("Bapophysis — Preferences")
                .with_inner_size([500., 650.])
                .with_min_inner_size([420., 400.])
                .with_window_level(egui::WindowLevel::Normal),
            |ctx, _| {
                if ctx.input(|i| i.viewport().close_requested()) {
                    open = false;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().id_salt("preferences content").show(ui, |ui| {
                ui.heading("Rendering performance");
                if ui.add(egui::Slider::new(&mut self.preview_batches, 1..=32)
                    .text("GPU batch size")).changed() {
                    if let Err(e) = settings::save_render_batches(self.preview_batches) {
                        self.status = format!("Could not remember rendering settings: {e}");
                    }
                }
                ui.small(format!("{} iterations per preview update", self.preview_batches * PARTICLES * 32));
                ui.small("Lower: smoother interaction and more frequent updates. Higher: less transfer overhead, but longer waits between updates. Final quality is controlled by the iteration target.");
                ui.separator();
                ui.heading("Random generation");
                ui.add(egui::Slider::new(&mut self.batch_size, 1..=48).text("Flames per batch"));
                ui.small("Applies to the next batch. Smaller galleries generate fewer thumbnails.");
                ui.label("Choose which variations may appear in new flames.");
                ui.small("Affine positioning is always applied before the chosen variation.");
                let mut changed = false;
                ui.horizontal(|ui| {
                    if ui.button("Enable all").clicked() {
                        self.options.enabled.fill(true);
                        changed = true;
                    }
                    if ui.button("Linear only").clicked() {
                        self.options.enabled.fill(false);
                        self.options.enabled[0] = true;
                        changed = true;
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt("variation scroll")
                    .max_height(240.)
                    .show(ui, |ui| {
                        egui::Grid::new("variation choices")
                            .num_columns(2)
                            .spacing([20., 4.])
                            .show(ui, |ui| {
                                for (i, (name, description)) in
                                    settings::VARIATIONS.iter().enumerate()
                                {
                                    let can_toggle = !self.options.enabled[i]
                                        || self.options.enabled.iter().filter(|&&v| v).count() > 1;
                                    changed |= ui
                                        .add_enabled(
                                            can_toggle,
                                            egui::Checkbox::new(
                                                &mut self.options.enabled[i],
                                                *name,
                                            ),
                                        )
                                        .on_hover_text(*description)
                                        .changed();
                                    if i % 2 == 1 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
                ui.small("Keep at least one enabled. Choices are saved automatically and restored on launch. Existing previews stay unchanged.");
                if changed {
                    if let Err(e) = self.options.save() {
                        self.status =
                            format!("Preferences apply for this session; could not save them: {e}");
                    }
                }
                ui.separator();
                ui.heading("PNG export");
                let mut export_changed = ui
                    .checkbox(
                        &mut self.export_options.transparent,
                        "Transparent background",
                    )
                    .changed();
                ui.small("Transparency follows flame density, including soft wisps.");
                egui::ComboBox::from_id_salt("export resolution")
                    .selected_text(match self.export_options.resolution {
                        0 => "Preview size".to_string(),
                        1 => "Custom dimensions".to_string(),
                        edge => format!("{edge} px longest edge"),
                    })
                    .show_ui(ui, |ui| {
                        for (value, label) in [
                            (0, "Preview size"),
                            (1024, "1024 px longest edge"),
                            (1536, "1536 px longest edge"),
                            (2048, "2048 px longest edge"),
                            (1, "Custom dimensions"),
                        ] {
                            export_changed |= ui
                                .selectable_value(&mut self.export_options.resolution, value, label)
                                .changed();
                        }
                    });
                if self.export_options.resolution == 1 {
                    ui.horizontal(|ui| {
                        ui.label("Width");
                        export_changed |= ui
                            .add(
                                egui::DragValue::new(&mut self.export_options.width)
                                    .range(64..=2048),
                            )
                            .changed();
                        ui.label("Height");
                        export_changed |= ui
                            .add(
                                egui::DragValue::new(&mut self.export_options.height)
                                    .range(64..=2048),
                            )
                            .changed();
                    });
                    ui.small("Custom aspect ratio keeps vertical framing; width changes the crop.");
                }
                let size = self.export_options.dimensions(self.render_size);
                ui.label(format!("Output: {} × {} pixels", size[0], size[1]));
                ui.small("Larger outputs render separately; you can keep browsing.");
                if export_changed {
                    if let Err(e) = self.export_options.save() {
                        self.status = format!("Could not remember export preferences: {e}");
                    }
                }
                ui.separator();
                if ui.button("Generate a new batch").clicked() {
                    generate = true;
                }
                });
                });
            },
        );
        self.preferences_open = open;
        if generate {
            self.new_batch();
            self.preferences_open = false;
        }
    }
}
fn viewport_resolution(available: egui::Vec2, pixels_per_point: f32) -> [u32; 2] {
    // Render at physical window resolution, bounded by the supported GPU buffer size.
    let physical = available * pixels_per_point;
    let scale = (2048. / physical.x.max(physical.y).max(1.)).min(1.);
    [
        (physical.x * scale).round().clamp(1., 2048.) as u32,
        (physical.y * scale).round().clamp(1., 2048.) as u32,
    ]
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let previous_target = self.iteration_target;
        self.collect(ctx);
        egui::SidePanel::left("gallery")
            .exact_width(270.)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(8.);
                ui.heading("Bapophysis");
                ui.label("Random flame explorer");
                if ui.button("Preferences…").clicked() {
                    self.preferences_open = true;
                }
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
                if ui.button("Gradient…").clicked() {
                    self.gradient_open = true;
                }
                if ui
                    .add_enabled(self.samples < self.iteration_target,
                        egui::Button::new(if self.running { "Pause" } else { "Resume" }))
                    .clicked()
                {
                    self.running = !self.running;
                }
                if ui.button("Fit view").clicked() {
                    self.camera = self.previews[self.selected].flame.camera;
                    self.camera[2] =
                        self.camera[3] * self.render_size[0] as f32 / self.render_size[1] as f32;
                    self.samples = 0;
                    self.dirty = true;
                    self.running = true;
                }
                ui.add_sized(
                    [125., 18.],
                    egui::Label::new(format!("{:.1}M iterations", self.samples as f64 / 1e6)),
                );
                let old_target = self.iteration_target;
                egui::ComboBox::from_id_salt("iteration target")
                    .width(120.)
                    .selected_text("Iteration presets")
                    .show_ui(ui, |ui| {
                        for target in [32_000_000, 128_000_000, 512_000_000, 1_024_000_000] {
                            ui.selectable_value(
                                &mut self.iteration_target,
                                target,
                                format!("{} million", target / 1_000_000),
                            );
                        }
                    });
                ui.label("Target (millions):");
                let mut millions = self.iteration_target as f64 / 1_000_000.;
                if ui.add(egui::DragValue::new(&mut millions)
                    .range(0.000001..=10_000.)
                    .speed(1.)
                    .max_decimals(6)
                    .update_while_editing(false))
                    .on_hover_text("Type millions of iterations: 1500 = 1.5 billion. Maximum 10,000 million. Press Enter to apply.")
                    .changed() {
                    self.iteration_target = (millions * 1_000_000.).round() as u64;
                }
                if self.iteration_target != old_target {
                    self.running = self.samples < self.iteration_target;
                }
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
                if ui
                    .add_enabled(
                        self.samples > 0 && !self.dirty && self.export_job.is_none(),
                        egui::Button::new("Save PNG…"),
                    )
                    .clicked()
                {
                    self.export();
                }
                let size = self.export_options.dimensions(self.render_size);
                ui.small(format!(
                    "{} × {} · {}",
                    size[0],
                    size[1],
                    if self.export_options.transparent {
                        "Transparent"
                    } else {
                        "Background"
                    }
                ));
                if let Some(job) = &self.export_job {
                    ui.label(format!(
                        "Export {:.0}%",
                        (job.samples as f64 / job.target as f64 * 100.).min(100.)
                    ));
                    if ui.button("Cancel export").clicked() {
                        self.export_job = None;
                        self.status = "Export cancelled; no file was written.".into();
                    }
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
            let available = ui.available_size().max(egui::vec2(1., 1.));
            let size = viewport_resolution(available, ctx.pixels_per_point());
            if size != self.render_size {
                self.render_size = size;
                self.camera[2] = self.camera[3] * size[0] as f32 / size[1] as f32;
                self.samples = 0;
                self.dirty = true;
                self.running = true;
            }
            let (view, response) = ui.allocate_exact_size(available, egui::Sense::drag());
            if response.dragged() {
                let delta = ctx.input(|i| i.pointer.delta());
                self.camera[0] -= delta.x / available.x * self.camera[2];
                self.camera[1] += delta.y / available.y * self.camera[3];
                self.dirty = true;
                self.samples = 0;
                self.running = true;
            }
            if response.hovered() {
                let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0. {
                    let extent = (self.camera[3] * (-scroll * 0.002).exp()).clamp(0.001, 1000.);
                    self.camera[2] =
                        extent * self.render_size[0] as f32 / self.render_size[1] as f32;
                    self.camera[3] = extent;
                    self.dirty = true;
                    self.samples = 0;
                    self.running = true;
                }
            }
            if let Some(texture) = &self.texture {
                let image = reproject_preview(view, self.texture_camera, self.camera);
                ui.painter().with_clip_rect(view).image(
                    texture.id(),
                    image,
                    egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(1., 1.)),
                    egui::Color32::WHITE,
                );
            } else {
                ui.put(view, egui::Spinner::new());
            }
        });
        self.preferences(ctx);
        self.gradient_popup(ctx);
        if self.iteration_target != previous_target {
            if let Err(e) = settings::save_iteration_target(self.iteration_target) {
                self.status = format!("Could not remember iteration target: {e}");
            }
        }
        self.schedule();
        if self.running
            || self.export_job.is_some()
            || self.detail.pending.is_some()
            || self.thumbnails.pending.is_some()
            || self.dirty
        {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}
fn reproject_preview(view: egui::Rect, source: [f32; 4], camera: [f32; 4]) -> egui::Rect {
    let centre = view.center()
        + egui::vec2(
            (source[0] - camera[0]) / camera[2] * view.width(),
            (camera[1] - source[1]) / camera[3] * view.height(),
        );
    egui::Rect::from_center_size(
        centre,
        egui::vec2(
            view.width() * source[2] / camera[2],
            view.height() * source[3] / camera[3],
        ),
    )
}
fn accumulate(total: &mut [[u64; 4]], batch: &[[u32; 4]]) {
    assert_eq!(total.len(), batch.len());
    for (total, batch) in total.iter_mut().zip(batch) {
        for c in 0..4 {
            total[c] += batch[c] as u64;
        }
    }
}
fn gradient_strip(
    ui: &mut egui::Ui,
    palette: &gradients::Palette,
    size: egui::Vec2,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    for i in 0..256 {
        let colour = gradients::sample(palette, i as f32 / 255.);
        let segment = egui::Rect::from_min_max(
            rect.min + egui::vec2(rect.width() * i as f32 / 256., 0.),
            rect.min + egui::vec2(rect.width() * (i + 1) as f32 / 256. + 0.5, rect.height()),
        );
        ui.painter().rect_filled(
            segment,
            0.,
            egui::Color32::from_rgb(
                (colour[0] * 255.) as u8,
                (colour[1] * 255.) as u8,
                (colour[2] * 255.) as u8,
            ),
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
fn export_camera(mut camera: [f32; 4], size: [u32; 2]) -> [f32; 4] {
    camera[2] = camera[3] * size[0] as f32 / size[1] as f32;
    camera
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
    let mut transform = generate(1).transforms[0];
    transform.row0 = [1., 0., 0., 1.];
    transform.row1 = [0., 1., 0., 1.];
    transform.parameters = [3., 0.5, 1.3, -0.7];
    let mut verification = generate(1).params(THUMB, THUMB);
    verification.transforms[0] = transform;
    gpu.reset(&verification);
    let actual = gpu.verify_variations();
    for (index, point) in actual.iter().enumerate() {
        let kind = index % settings::VARIATION_COUNT;
        transform.variation = [kind as f32, 1., 0., 0.];
        let expected = if index < settings::VARIATION_COUNT {
            flame::apply(&transform, [0.7, -0.4])
        } else {
            flame::apply_random(&transform, [0.7, -0.4], [0.57, 0.72, 0.66, 0.44, 0.71])
        };
        for axis in 0..2 {
            assert!(
                (point[axis] - expected[axis]).abs() < 1e-4,
                "CPU/GPU variation mismatch: {kind}"
            );
        }
    }
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
    // Clearing the short GPU histogram must preserve orbit state and total density.
    gpu.dispatch();
    let expected = await_pixels(&mut gpu);
    gpu.reset(&flame.params(THUMB, THUMB));
    gpu.dispatch();
    let a = await_pixels(&mut gpu);
    gpu.clear_histogram();
    gpu.dispatch();
    let b = await_pixels(&mut gpu);
    let mut sum = vec![[0u64; 4]; a.len()];
    accumulate(&mut sum, &a);
    accumulate(&mut sum, &b);
    assert!(
        sum.iter()
            .zip(&expected)
            .all(|(a, b)| (0..4).all(|c| a[c] == b[c] as u64)),
        "Chunked accumulation lost orbit state or samples"
    );
    let mut recoloured = flame.params(THUMB, THUMB);
    recoloured.palette = gradients::palette(if flame.palette == gradients::palette(2) {
        1
    } else {
        2
    });
    gpu.reset(&recoloured);
    gpu.dispatch();
    let coloured = await_pixels(&mut gpu);
    assert!(
        coloured.iter().zip(&a).all(|(new, old)| new[0] == old[0]),
        "Gradient changed fractal geometry"
    );
    assert!(
        coloured
            .iter()
            .zip(&a)
            .any(|(new, old)| new[1..] != old[1..]),
        "Gradient did not change colours"
    );
    // A square affine attractor fills a panned rectangular view, including corners.
    // This catches accidental radial clipping or a stale camera mapping.
    let mut square = generate(1);
    for (i, t) in square.transforms.iter_mut().enumerate() {
        t.row0 = [0.5, 0., if i % 2 == 0 { -0.5 } else { 0.5 }, 1.];
        t.row1 = [0., 0.5, if i < 2 { -0.5 } else { 0.5 }, (i + 1) as f32 / 4.];
        t.variation = [0., 0., i as f32 / 3., 0.];
    }
    square.camera = [0.5, 0., 1., 2.];
    let mut rectangle = Gpu::new(device.clone(), queue.clone(), 192, 96);
    rectangle.reset(&square.params(192, 96));
    rectangle.dispatch();
    let pixels = await_pixels(&mut rectangle);
    for (x, y) in [(0, 0), (176, 0), (0, 80), (176, 80)] {
        let hits: u32 = (y..y + 16)
            .flat_map(|row| (x..x + 16).map(move |col| row * 192 + col))
            .map(|i| pixels[i][0])
            .sum();
        assert!(hits > 50, "Panned rectangular corner was clipped");
    }
    write_png_rect("exports/check-rectangle.png", 192, 96, &pixels);
    // Exercise the same batched rendering and RGBA encoder used by large exports.
    let mut high_resolution = Gpu::new(device.clone(), queue.clone(), 2048, 1024);
    let mut p = generate(1).params(2048, 1024);
    p.camera = export_camera(p.camera, [2048, 1024]);
    high_resolution.reset(&p);
    high_resolution.dispatch_batches(EXPORT_BATCHES);
    let pixels = await_pixels(&mut high_resolution);
    assert_eq!(pixels.len(), 2048 * 1024);
    let path = std::path::PathBuf::from(format!(
        "exports/check-alpha-{}.png",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    export::save_png(&path, 2048, 1024, &pixels, 1., 1, true).unwrap();
    let data = std::fs::read(&path).unwrap();
    let mut reader = png::Decoder::new(data.as_slice()).read_info().unwrap();
    assert_eq!((reader.info().width, reader.info().height), (2048, 1024));
    assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    let mut bytes = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut bytes).unwrap();
    assert!(bytes.chunks_exact(4).any(|p| p[3] == 0));
    assert!(bytes.chunks_exact(4).any(|p| p[3] > 0 && p[3] < 255));
    let mut detail = Gpu::new(device, queue, SIZE, SIZE);
    detail.reset(&flame.params(SIZE, SIZE));
    let mut final_pixels = Vec::new();
    for _ in 0..16 {
        detail.dispatch();
        final_pixels = await_pixels(&mut detail);
    }
    write_png("exports/check-detail.png", SIZE, &final_pixels);
    println!(
        "GPU check passed: {} | 40 formulas and random branches, 12 previews, continued accumulation, panned corners, 2048 × 1024 RGBA export",
        adapter.get_info().name
    );
}
fn write_png_rect(path: &str, width: u32, height: u32, pixels: &[[u32; 4]]) {
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&colours(pixels, 1.))
        .unwrap();
}
#[cfg(test)]
mod viewport_tests {
    use super::*;
    #[test]
    fn navigation_preview_tracks_pan_zoom_and_clipping() {
        let view = egui::Rect::from_min_size(egui::pos2(10., 20.), egui::vec2(800., 600.));
        let source = [0., 0., 8., 6.];
        assert_eq!(reproject_preview(view, source, source), view);
        let pan = reproject_preview(view, source, [1., 1., 8., 6.]);
        assert_eq!(pan.min, view.min + egui::vec2(-100., 100.));
        let zoom = reproject_preview(view, source, [0., 0., 4., 3.]);
        assert_eq!(zoom.center(), view.center());
        assert_eq!(zoom.size(), view.size() * 2.);
        assert_eq!(zoom.intersect(view), view);
    }
    #[test]
    fn rectangular_resolution_fills_view_with_bounded_pixel_cost() {
        assert_eq!(
            viewport_resolution(egui::vec2(1200., 600.), 1.),
            [1200, 600]
        );
        assert_eq!(
            viewport_resolution(egui::vec2(600., 1200.), 1.),
            [600, 1200]
        );
        assert_eq!(
            viewport_resolution(egui::vec2(800., 600.), 2.),
            [1600, 1200]
        );
        assert_eq!(
            viewport_resolution(egui::vec2(2400., 1200.), 1.),
            [2048, 1024]
        );
    }
    #[test]
    fn export_resolution_and_camera_preserve_view() {
        let mut options = settings::ExportOptions {
            resolution: 1536,
            ..Default::default()
        };
        assert_eq!(options.dimensions([768, 384]), [1536, 768]);
        assert_eq!(
            export_camera([1., 2., 4., 2.], [1536, 768]),
            [1., 2., 4., 2.]
        );
        options.resolution = 1;
        options.width = 4000;
        options.height = 1;
        assert_eq!(options.dimensions([768, 384]), [2048, 64]);
    }
    #[test]
    fn extended_accumulation_preserves_values_above_u32() {
        let mut totals = vec![[u32::MAX as u64; 4]];
        accumulate(&mut totals, &[[123, 456, 789, 999]]);
        assert_eq!(
            totals[0],
            [
                u32::MAX as u64 + 123,
                u32::MAX as u64 + 456,
                u32::MAX as u64 + 789,
                u32::MAX as u64 + 999
            ]
        );
        assert_eq!(colours(&totals, 1.).len(), 3);
    }
}
