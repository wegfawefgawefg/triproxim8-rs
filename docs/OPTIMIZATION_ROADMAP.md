# Optimization Roadmap

## Context
Current implementation is a fast baseline (CPU raster + CPU loss, decoupled from render FPS), but there is substantial additional headroom if we optimize architecture, not just micro-ops.

## Goal Direction
Target outcomes discussed:
- much higher search throughput,
- higher-resolution comparison during optimization,
- potential path toward extreme gains (100x to 1000x class, hardware dependent).

## Tier 1: High-Impact CPU Upgrades (Near-Term)
These are practical and low-risk relative to current code.

1. Batch offspring per iteration (`lambda` mutants)
- Generate many mutated candidates from the current best state each step.
- Evaluate all candidates, keep the best one.
- This improves useful work per synchronization point.

2. Parallel candidate evaluation (`rayon`)
- Evaluate offspring in parallel across CPU cores.
- Keep UI/render thread separate from optimizer loop.

3. Early-abort loss computation
- While comparing pixels, stop if partial loss already exceeds current best.
- Most bad candidates get rejected early, reducing average compute per evaluation.

4. SIMD-accelerated pixel difference
- Use SIMD-friendly loops for absolute RGB difference accumulation.
- Helps especially when early-abort does not trigger quickly.

5. Multi-resolution schedule
- Optimize at low resolution first (example: `32 -> 64 -> 128`).
- Promote candidate/population to higher resolution after loss stabilizes.

Expected impact (combined): typically ~10x to ~50x depending CPU and mutation regime.

## Tier 2: GPU Search Pipeline (Major Step)
For very large throughput jumps, push both rendering and loss to the GPU.

1. Batch render large candidate sets on GPU
- Rasterize thousands of candidate triangle sets per dispatch/frame.

2. GPU-side loss reduction
- Compute candidate loss on GPU and reduce to best index/loss.
- CPU receives only minimal results.

3. Keep visualization separate
- Continue showing only selected best candidate in the UI.
- Optimizer is not bound to display rate.

Expected impact: additional ~20x to ~200x over CPU path in favorable conditions.
Combined with Tier 1 can approach 100x to 1000x class speedups on strong hardware.

## Tier 3: Better Search Algorithms (Quality per Compute)
Raw speed helps, but search strategy often dominates final quality.

1. Move beyond single-incumbent hill-climbing
- Use `(mu, lambda)-ES`, CMA-ES, or cross-entropy method.
- Maintain population/archive instead of one best state.

2. Structured mutations
- Separate geometry-local, color-local, and ordering/topology mutations.
- Use adaptive mutation scales instead of fixed deltas.

3. Acceptance and scheduling logic
- Annealing-style schedules or adaptive step sizes.
- Restart strategies when progress stalls.

Expected impact: better quality and convergence for the same compute budget.

## About "DNA Generates Params Generates Triangles"
This is a valid direction (procedural/indirect encodings like CPPN/grammar/hypernetwork style), but it is not automatically faster.

Pros:
- More compressed representation.
- Better global structure/regularity possible.
- Can capture style-like priors.

Cons:
- Harder optimization landscape.
- Additional model complexity can slow iteration.
- May require hybrid training/search to be effective.

Recommended use:
- Hybrid pipeline: coarse indirect generator for layout + direct triangle refinement stage.

## Practical Next Implementation Step
If we want immediate measurable gains without a full rewrite:

1. Add batched offspring evaluation.
2. Add `rayon` parallel evaluation.
3. Add early-abort loss.
4. Add multi-resolution optimization mode.
5. Add benchmark counters (`evals/sec`, `steps/sec`, per-stage timing).

This gives concrete metrics first, then informs whether GPU work is worth the complexity.
