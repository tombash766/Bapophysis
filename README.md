# Bapophysis

A lightweight native Windows GPU fractal-flame explorer, built with Rust, egui
and wgpu/DirectX 12. Requires compatible graphics hardware and current drivers.

## Run

Open `target/release/bapophysis.exe`, or build with `cargo run --release`.
Building requires Rust and the Visual Studio C++ build tools with Windows SDK.

## Explore

- A fresh batch of 24 random flames is generated at startup.
- The scrollable gallery on the left shows fast 160 x 160 previews in two columns.
- Click a preview to immediately show it, then progressively render that same
  flame at 768 x 768. The first candidate is selected automatically.
- Choose 12, 24 or 48 flames and press New batch to explore another set.
  This replaces the current batch; save any images you want to keep first.
- Pause/Refine controls detailed rendering. Fit view restores automatic framing.
- Drag the large image to pan; scroll over it to zoom.
- Exposure changes the detailed image's brightness without restarting rendering.
- Save image exports a PNG to the filename shown, protecting existing files.

## Flames

Each flame has four randomized affine transforms, selection weights, colour
coordinates and a three-stop palette. Each transform blends its affine result
with Linear, Sinusoidal, Spherical, Swirl or Horseshoe. Spherical and Horseshoe
use a softened radius near the origin for stability. A short CPU orbit probe
rejects collapsed/escaping candidates and estimates a square camera frame.
The GPU generates both thumbnails and detailed images using the same parameters.
A thumbnail uses one short sampling batch; detailed rendering accumulates up to
approximately 32 million iterations. Selecting a different flame resets detail.

Each GPU renderer uses 8,192 independent seeded orbits in batches of 32 steps,
with 19 initial burn-in steps. Density and fixed-point RGB sums accumulate on the
GPU. Asynchronous readback, logarithmic density mapping and CPU image encoding
produce the preview and exported image. The sidebar remains usable while
previews are being filled. There is no bundled browser runtime.

This version prioritizes quick exploration. Some candidates will be less
interesting than others, framing may omit rare outliers, and previews are noisy.
Batches are not persisted between app launches. PNG metadata includes the flame's
internal seed; loading projects or seeds is not yet exposed in the interface.

## Verify

```powershell
cargo test --locked
cargo clippy --locked -- -D warnings
cargo run --release --locked -- --gpu-check
```

The GPU check renders twelve random coloured thumbnails, verifies coverage,
checks deterministic reset and colour accumulation, and renders a larger image.
Images are written under `exports/check-flame-*.png` and `exports/check-detail.png`.
