# triproxim8-rs Goal

## Objective
Reimplement `triproxim8-py` in Rust with raylib while keeping the same core behavior:
- approximate a target image with many colored triangles,
- use a mutation-based search that keeps the best candidate seen so far,
- render target and approximation side-by-side in real time.

## Why This Rust Port Exists
The Python version is easy to iterate on but limited in simulation throughput. This Rust version is intended to push search speed much higher by keeping the hot loop CPU-local and minimizing expensive per-frame work.

## Performance Strategy
- Comparison resolution stays tiny (`32x32`) so each evaluation is cheap.
- Triangle rasterization and loss comparison happen in plain Rust buffers (`Vec<u8>`), not through readbacks from GPU textures.
- Simulation work is decoupled from display work by running many evolution steps per rendered frame using a configurable time budget and cap.
- Rendering is only for visualization; evolution can continue at higher throughput than display refresh.

## Runtime UX Targets
- Fast controls for mutation rate and simulation budget.
- Toggle/pause and reset behavior for quick experiments.
- Multiple target assets (ported from the Python repo).
- Export action that saves:
  - the current best approximation image,
  - the exact gene data as JSON triangle configuration for later high-res re-rendering.

## Code Shape Constraints
- Keep source files in a manageable, C++-leaning style with explicit structs/functions and straightforward data flow.
- Avoid over-fragmenting tiny abstractions; keep performance-critical logic easy to inspect.
- Keep file sizes in the requested practical range instead of generating overly large monolithic files.

## Success Criteria
- Running `cargo run --release` starts an interactive window.
- Running `cargo run --release -- --rerender <json> --width <w> --height <h> --out <png>` renders exported JSON at higher resolution without opening the window.
- Evolution visibly improves approximation over time.
- Search can perform a large number of simulation steps per frame (configurable).
- Export produces reproducible output artifacts in `exports/`.
