# Bapophysis

A native Windows GPU fractal-flame explorer built with Rust, egui and wgpu/DirectX 12.
Requires compatible graphics hardware and current graphics drivers.

## Run

Open `target/release/bapophysis.exe` for the latest build. `build/release` also
contains a separate test build, created to preserve a running older executable.

```powershell
cargo run --release --locked
```

Building requires Rust and Visual Studio C++ build tools with the Windows SDK.

## Explore

A fresh batch of 24 random flames is generated at startup. The scrollable gallery
shows fast 160 x 160 previews. Select one to render the same flame progressively
across the full rectangular viewing area. The preview's longest edge is 768 pixels.
Choose 12, 24 or 48 flames and press New batch to replace the batch.

Drag the image to pan, scroll to zoom, and use Fit view to restore framing.
Exposure changes brightness without restarting sampling. Pause/Refine controls
rendering. Batches are not saved between app launches; export images you want to keep.

## Forty variations

Preferences contains a scrollable list of all supported variations:

Linear, Sinusoidal, Spherical, Swirl, Horseshoe, Polar, Handkerchief, Heart, Disc,
Spiral, Hyperbolic, Diamond, Ex, Bent, Fisheye, Eyefish, Bubble, Cylinder, Julia,
Waves, Popcorn, Exponential, Power, Cosine, Rings, Fan, Noise, Blur, Gaussian blur,
Pre blur, Radial blur, Rings2, Fan2, Blob, PDJ, Perspective, Julian, Juliascope,
Curl and Rectangles.

Keep at least one enabled. Choices affect new batches and are remembered under
%LOCALAPPDATA%/Bapophysis. Existing five- and eighteen-variation preferences are
migrated without changing their choices; new entries start unchecked. Enable all
includes the new entries. If Linear is disabled, nonlinear transforms use no
implicit linear blend. Affine positioning always precedes the selected variation,
except that Pre blur adds jitter before the affine map.

Parameterized variations receive random parameters per transform. Stochastic
variations use seeded random inputs in both CPU probes and GPU rendering.
Definitions were checked against the [original flam3 reference](https://github.com/scottdraves/flam3).
Singular denominators and extreme exponentials are guarded for stability.
This is not Apophysis file-format or exact rendering compatibility.

## Gradients per fractal

The main toolbar has one Gradient button, opening a separate pop-up. Its live
fractal preview and full-width gradient strip show the selected fractal's colours.
The Rotate slider shifts the palette without changing its colour content.

Random gradients now use 256 colour entries interpolated between 16–32 saturated,
randomly spaced colour stops. Bright highlights and occasional dark bands create
richer, more varied colour transitions. Every new fractal receives a seeded random
palette. New random gradient generates a fresh palette rather than selecting a
fixed preset. Click any of the ten random preview strips to apply it; Shuffle
choices replaces the list. Generating the same seed reproduces the same palette.

Classic presets remain available in the pop-up: Ember, Ocean, Aurora, Sunset,
Orchid, Forest, Ice, Copper, Candy, Monochrome, Solar and Twilight. Restore original
returns to that fractal's initial generated palette and resets rotation.

Changes affect only the selected fractal and refresh its thumbnail and detailed
render while preserving geometry and framing. Its palette and rotation are kept
when switching between fractals in a batch. Exports already underway retain their
captured palette. Colour sampling on the GPU uses the same 256-entry palette
shown by the gradient strips.

## More iterations

Choose a target of 32M, 128M, 512M or 1024M. Increasing it resumes accumulation
without discarding existing samples. +128M adds another 128 million iterations,
up to ten billion. Refine at a completed target also adds 128M. Targets belong
to individual fractals in the batch. Selecting another fractal starts its render
again; pan, zoom, Fit view and gradient changes also restart sampling for the new view.

Higher iteration counts take proportionally longer. Sparse zoomed areas benefit
as more generated points land inside the current view. There is no circular mask;
round edges can be part of the attractor. Orbits still evolve outside the camera
to preserve the IFS distribution, but only visible points are stored.

## PNG output

Preferences offers a Transparent background checkbox; preview resolution;
1024/1536/2048-pixel longest-edge presets; and custom dimensions from 64 to 2048
pixels per side. Presets retain the view's aspect ratio. Custom aspect ratios
preserve vertical framing and change the horizontal crop. Settings are remembered.

Save PNG opens a native Windows Save dialog. Existing files are protected.
Transparent exports use straight RGBA: untouched pixels have zero alpha, and
wisps have density-based partial alpha. The interactive preview retains its background.

Different output dimensions start a separate GPU render with the flame, camera,
exposure and transparency captured at save time. Browsing or changing settings
does not affect that export. Export sampling uses at least the selected fractal's
iteration target. Progress and Cancel export appear beside Save PNG; cancellation
writes no file. PNG encoding completes before a file is created and may briefly
pause the interface. Write errors include the destination path.

## Rendering and verification

The GPU evolves 8,192 independent seeded orbits in batches of 32 steps, with
19 initial burn-in steps. Short integer density and RGB batches merge into 64-bit
CPU totals, then the histogram is cleared without resetting the orbits. This
avoids colour overflow during long preview and export renders. Exports read back
every eight batches to reduce transfer overhead. Tone mapping uses logarithmic
density, palette colours and exposure. No browser runtime is bundled.

At the maximum export resolution, histogram/readback buffers use about 128 MiB
of GPU memory. CPU accumulation uses another 128 MiB plus image-encoding memory.

```powershell
cargo test --locked
cargo clippy --locked -- -D warnings
cargo run --release --locked -- --gpu-check
```

Tests check variation selection, deterministic generation, formulas, gradients,
large accumulation totals, resolution/camera mapping, and PNG RGB/RGBA round trips.
The GPU check compares all forty formulas with two random inputs, renders twelve
previews, verifies continued accumulation and palette-independent geometry,
checks panned rectangular corners, and decodes a 2048 x 1024 transparent export.
Test images are written under `exports/`.

