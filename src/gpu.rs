use crate::flame::Params;
use std::sync::mpsc;
use wgpu::util::DeviceExt;
pub const PARTICLES: u32 = 8192;
fn initial_particles() -> Vec<[u32; 8]> {
    (0..PARTICLES)
        .map(|i| {
            [
                0,
                0,
                (i + 1).wrapping_mul(747796405) | 1,
                0,
                0.5f32.to_bits(),
                0,
                0,
                0,
            ]
        })
        .collect()
}
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bindings: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    particles: wgpu::Buffer,
    histogram: wgpu::Buffer,
    readback: wgpu::Buffer,
    pub pending: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
}
impl Gpu {
    pub fn clear_histogram(&self) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.clear_buffer(&self.histogram, 0, None);
        self.queue.submit(Some(encoder.finish()));
    }
    pub fn verify_variations(&self) -> Vec<[f32; 2]> {
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Variation verification"),
                source: wgpu::ShaderSource::Wgsl(include_str!("flame.wgsl").into()),
            });
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&self.pipeline.get_bind_group_layout(0)],
                push_constant_ranges: &[],
            });
        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&layout),
                module: &shader,
                entry_point: Some("check_variations"),
                compilation_options: Default::default(),
                cache: None,
            });
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: self.particles.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &self.bindings, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.particles, 0, &output, 0, output.size());
        self.queue.submit(Some(encoder.finish()));
        let (tx, rx) = mpsc::channel();
        output.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();
        let result = {
            let data = output.slice(..).get_mapped_range();
            bytemuck::cast_slice::<u8, [u32; 8]>(&data)
                .iter()
                .take(crate::settings::VARIATION_COUNT * 2)
                .map(|p| [f32::from_bits(p[0]), f32::from_bits(p[1])])
                .collect()
        };
        output.unmap();
        result
    }
    pub fn resized(&self, width: u32, height: u32) -> Self {
        Self::new(self.device.clone(), self.queue.clone(), width, height)
    }
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, width: u32, height: u32) -> Self {
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Flame parameters"),
            contents: bytemuck::bytes_of(&crate::flame::generate(1).params(width, height)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let particles = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Independent orbits"),
            contents: bytemuck::cast_slice(&initial_particles()),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let histogram = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Density"),
            size: (width * height * 16) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Preview readback"),
            size: histogram.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Flame compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("flame.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Flame renderer"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: particles.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: histogram.as_entire_binding(),
                },
            ],
        });
        Self {
            device,
            queue,
            pipeline,
            bindings,
            uniform,
            particles,
            histogram,
            readback,
            pending: None,
        }
    }
    pub fn reset(&mut self, p: &Params) {
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(p));
        self.queue.write_buffer(
            &self.particles,
            0,
            bytemuck::cast_slice(&initial_particles()),
        );
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.clear_buffer(&self.histogram, 0, None);
        self.queue.submit(Some(encoder.finish()));
    }
    pub fn dispatch(&mut self) {
        self.dispatch_batches(1);
    }
    pub fn dispatch_batches(&mut self, batches: u32) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        for _ in 0..batches {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bindings, &[]);
            pass.dispatch_workgroups(PARTICLES / 128, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.histogram, 0, &self.readback, 0, self.histogram.size());
        self.queue.submit(Some(encoder.finish()));
        let (tx, rx) = mpsc::channel();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.pending = Some(rx);
    }
    pub fn collect(&mut self) -> Option<Result<Vec<[u32; 4]>, String>> {
        self.device.poll(wgpu::Maintain::Poll);
        match self.pending.as_ref()?.try_recv() {
            Ok(Ok(())) => {
                self.pending = None;
                let values = {
                    let bytes = self.readback.slice(..).get_mapped_range();
                    bytemuck::cast_slice::<u8, [u32; 4]>(&bytes).to_vec()
                };
                self.readback.unmap();
                Some(Ok(values))
            }
            Ok(Err(e)) => {
                self.pending = None;
                Some(Err(e.to_string()))
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                Some(Err("GPU readback disconnected".into()))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }
}
