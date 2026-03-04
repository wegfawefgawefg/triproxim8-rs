use image::{RgbImage, imageops::FilterType};
use rand::{Rng, SeedableRng, rngs::SmallRng};
use raylib::prelude::*;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod rerender;

const COMP_W: usize = 32; const COMP_H: usize = 32; const PIX_STRIDE: usize = 3;
const PIX_COUNT: usize = COMP_W * COMP_H; const BUF_LEN: usize = PIX_COUNT * PIX_STRIDE;
const DEFAULT_GENES: usize = 256; const DEFAULT_MUTATION: f32 = 0.001;
const MIN_MUTATION: f32 = 0.000001; const MAX_MUTATION: f32 = 0.5;
const DEFAULT_BUDGET_MS: f64 = 12.0; const MIN_BUDGET_MS: f64 = 0.0; const MAX_BUDGET_MS: f64 = 60.0;
const DEFAULT_STEP_CAP: usize = 200_000; const MIN_STEP_CAP: usize = 1_000; const MAX_STEP_CAP: usize = 2_000_000;
const VIEW_SCALE: i32 = 14; const PAD: i32 = 20; const PANEL_W: i32 = 390; const TITLE: &str = "triproxim8-rs";

#[derive(Clone, Copy, Debug)]
struct Gene {
    data: [f32; 9],
}

impl Gene {
    fn random(rng: &mut SmallRng) -> Self {
        let mut g = Self { data: [0.0; 9] };
        for v in &mut g.data {
            *v = rng.gen_range(0.0..=1.0);
        }
        g
    }
}

#[derive(Clone)]
struct TargetAsset {
    name: String,
    path: PathBuf,
    pixels: Vec<u8>,
}

struct Evolver {
    genes: Vec<Gene>,
    best_genes: Vec<Gene>,
    working_pixels: Vec<u8>,
    best_pixels: Vec<u8>,
    target_pixels: Vec<u8>,
    mutation_rate: f32,
    best_loss: u64,
    last_loss: u64,
    total_steps: u64,
    accepted_steps: u64,
    rejected_steps: u64,
    rng: SmallRng,
}

impl Evolver {
    fn new(target_pixels: Vec<u8>, gene_count: usize, mutation_rate: f32) -> Self {
        let mut rng = SmallRng::from_entropy();
        let genes = (0..gene_count)
            .map(|_| Gene::random(&mut rng))
            .collect::<Vec<_>>();

        let mut core = Self {
            genes: genes.clone(),
            best_genes: genes,
            working_pixels: vec![0; BUF_LEN],
            best_pixels: vec![0; BUF_LEN],
            target_pixels,
            mutation_rate,
            best_loss: u64::MAX,
            last_loss: u64::MAX,
            total_steps: 0,
            accepted_steps: 0,
            rejected_steps: 0,
            rng,
        };
        core.seed_best_from_current();
        core
    }

    fn seed_best_from_current(&mut self) {
        draw_genes_to_buffer(&self.genes, &mut self.working_pixels);
        let loss = compute_loss(&self.working_pixels, &self.target_pixels);
        self.best_loss = loss;
        self.last_loss = loss;
        self.best_genes.clone_from(&self.genes);
        self.best_pixels.clone_from(&self.working_pixels);
    }

    fn reset_random(&mut self, gene_count: usize) {
        self.genes.clear();
        self.genes.reserve(gene_count);
        for _ in 0..gene_count {
            self.genes.push(Gene::random(&mut self.rng));
        }
        self.total_steps = 0;
        self.accepted_steps = 0;
        self.rejected_steps = 0;
        self.best_loss = u64::MAX;
        self.last_loss = u64::MAX;
        self.seed_best_from_current();
    }

    fn set_target(&mut self, target_pixels: Vec<u8>, gene_count: usize) {
        self.target_pixels = target_pixels;
        self.reset_random(gene_count);
    }

    fn step(&mut self) {
        draw_genes_to_buffer(&self.genes, &mut self.working_pixels);
        let loss = compute_loss(&self.working_pixels, &self.target_pixels);
        self.last_loss = loss;

        if loss <= self.best_loss {
            self.best_loss = loss;
            self.best_genes.clone_from(&self.genes);
            self.best_pixels.clone_from(&self.working_pixels);
            self.accepted_steps += 1;
        } else {
            self.genes.clone_from(&self.best_genes);
            self.rejected_steps += 1;
        }

        self.mutate_all_genes();
        self.total_steps += 1;
    }

    fn mutate_all_genes(&mut self) {
        for g in &mut self.genes {
            for v in &mut g.data {
                mutate_component(v, self.mutation_rate, &mut self.rng);
            }
        }

        let n = self.genes.len();
        if n <= 1 {
            return;
        }
        for i in 0..n {
            if self.rng.r#gen::<f32>() < self.mutation_rate {
                self.genes.swap(i, self.rng.gen_range(0..n));
            }
        }
    }
}

fn mutate_component(v: &mut f32, mutation_rate: f32, rng: &mut SmallRng) {
    let chance = rng.r#gen::<f32>();
    if chance >= mutation_rate {
        return;
    }
    *v = (*v + rng.gen_range(-0.1..=0.1)).clamp(0.0, 1.0);
    if chance < (mutation_rate * 0.5) {
        *v = rng.gen_range(0.0..=1.0);
    }
}

fn draw_genes_to_buffer(genes: &[Gene], out: &mut [u8]) {
    out.fill(0);
    for g in genes {
        rasterize_triangle(out, g);
    }
}

fn rasterize_triangle(out: &mut [u8], gene: &Gene) {
    let x1 = gene.data[0] * COMP_W as f32;
    let y1 = gene.data[1] * COMP_H as f32;
    let x2 = gene.data[2] * COMP_W as f32;
    let y2 = gene.data[3] * COMP_H as f32;
    let x3 = gene.data[4] * COMP_W as f32;
    let y3 = gene.data[5] * COMP_H as f32;

    let r = (gene.data[6] * 255.0).clamp(0.0, 255.0) as u8;
    let g = (gene.data[7] * 255.0).clamp(0.0, 255.0) as u8;
    let b = (gene.data[8] * 255.0).clamp(0.0, 255.0) as u8;

    let min_x = x1.min(x2).min(x3).floor().max(0.0) as i32;
    let min_y = y1.min(y2).min(y3).floor().max(0.0) as i32;
    let max_x = x1.max(x2).max(x3).ceil().min((COMP_W - 1) as f32) as i32;
    let max_y = y1.max(y2).max(y3).ceil().min((COMP_H - 1) as f32) as i32;
    if min_x > max_x || min_y > max_y {
        return;
    }

    let area = edge(x1, y1, x2, y2, x3, y3);
    if area.abs() <= 0.000001 {
        return;
    }

    for py in min_y..=max_y {
        let fy = py as f32 + 0.5;
        for px in min_x..=max_x {
            let fx = px as f32 + 0.5;
            let w0 = edge(x2, y2, x3, y3, fx, fy);
            let w1 = edge(x3, y3, x1, y1, fx, fy);
            let w2 = edge(x1, y1, x2, y2, fx, fy);
            let inside = if area >= 0.0 {
                w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0
            } else {
                w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0
            };
            if !inside {
                continue;
            }
            let idx = ((py as usize * COMP_W) + px as usize) * PIX_STRIDE;
            out[idx] = r;
            out[idx + 1] = g;
            out[idx + 2] = b;
        }
    }
}

#[inline]
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (px - ax) * (by - ay) - (py - ay) * (bx - ax)
}

#[inline]
fn compute_loss(a: &[u8], b: &[u8]) -> u64 {
    let mut loss = 0u64;
    for (av, bv) in a.iter().zip(b) {
        loss += (*av as i32 - *bv as i32).unsigned_abs() as u64;
    }
    loss
}

fn load_target_pixels(path: &Path) -> Result<Vec<u8>, String> {
    let resized = image::open(path)
        .map_err(|e| format!("{}: {}", path.display(), e))?
        .resize_exact(COMP_W as u32, COMP_H as u32, FilterType::Triangle)
        .to_rgb8();
    let raw = resized.into_raw();
    if raw.len() != BUF_LEN {
        return Err(format!("{}: invalid raw size {}", path.display(), raw.len()));
    }
    Ok(raw)
}

fn load_targets() -> Vec<TargetAsset> {
    let candidates = [
        ("Fish", PathBuf::from("assets/fish.jpg")),
        ("Mona Lisa", PathBuf::from("assets/mona_lisa.jpg")),
    ];

    let mut assets = Vec::new();
    for (name, path) in candidates {
        match load_target_pixels(&path) {
            Ok(pixels) => assets.push(TargetAsset {
                name: name.to_string(),
                path,
                pixels,
            }),
            Err(err) => eprintln!("Failed to load target {}: {}", name, err),
        }
    }
    assert!(!assets.is_empty(), "No target assets could be loaded.");
    assets
}

fn draw_rgb_buffer(d: &mut RaylibDrawHandle, pixels: &[u8], x: i32, y: i32, scale: i32, label: &str) {
    d.draw_text(label, x, y - 28, 24, Color::WHITE);
    for py in 0..COMP_H as i32 {
        for px in 0..COMP_W as i32 {
            let idx = ((py as usize * COMP_W) + px as usize) * PIX_STRIDE;
            let c = Color::new(pixels[idx], pixels[idx + 1], pixels[idx + 2], 255);
            d.draw_rectangle(x + px * scale, y + py * scale, scale, scale, c);
        }
    }
    d.draw_rectangle_lines(
        x - 2,
        y - 2,
        COMP_W as i32 * scale + 4,
        COMP_H as i32 * scale + 4,
        Color::RAYWHITE,
    );
}

fn point_in_rect(point: Vector2, rect: Rectangle) -> bool {
    point.x >= rect.x
        && point.x <= rect.x + rect.width
        && point.y >= rect.y
        && point.y <= rect.y + rect.height
}

fn export_best(evolver: &Evolver, target: &TargetAsset) -> Result<String, String> {
    fs::create_dir_all("exports").map_err(|e| format!("mkdir exports: {}", e))?;

    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs();

    let slug = target
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();

    let stem = format!("{}_{}_loss{}", slug, epoch, evolver.best_loss);
    let image_path = format!("exports/{}.png", stem);
    let json_path = format!("exports/{}.json", stem);

    RgbImage::from_raw(COMP_W as u32, COMP_H as u32, evolver.best_pixels.clone())
        .ok_or_else(|| "Failed to construct export image".to_string())?
        .save(&image_path)
        .map_err(|e| format!("save {}: {}", image_path, e))?;

    let mut json = String::with_capacity(1024 + evolver.best_genes.len() * 110);
    writeln!(&mut json, "{{").map_err(|e| e.to_string())?;
    writeln!(&mut json, "  \"format\": \"triproxim8-rs-v1\",").map_err(|e| e.to_string())?;
    writeln!(&mut json, "  \"target_name\": {:?},", target.name).map_err(|e| e.to_string())?;
    writeln!(&mut json, "  \"target_path\": {:?},", target.path.display().to_string())
        .map_err(|e| e.to_string())?;
    writeln!(&mut json, "  \"comparison_resolution\": [{}, {}],", COMP_W, COMP_H)
        .map_err(|e| e.to_string())?;
    writeln!(&mut json, "  \"best_loss\": {},", evolver.best_loss).map_err(|e| e.to_string())?;
    writeln!(&mut json, "  \"gene_count\": {},", evolver.best_genes.len()).map_err(|e| e.to_string())?;
    writeln!(&mut json, "  \"triangles\": [").map_err(|e| e.to_string())?;
    for (i, g) in evolver.best_genes.iter().enumerate() {
        let trailing = if i + 1 == evolver.best_genes.len() { "" } else { "," };
        writeln!(
            &mut json,
            "    [{:.7}, {:.7}, {:.7}, {:.7}, {:.7}, {:.7}, {:.7}, {:.7}, {:.7}]{}",
            g.data[0], g.data[1], g.data[2], g.data[3], g.data[4], g.data[5], g.data[6], g.data[7],
            g.data[8], trailing
        )
        .map_err(|e| e.to_string())?;
    }
    writeln!(&mut json, "  ]").map_err(|e| e.to_string())?;
    writeln!(&mut json, "}}").map_err(|e| e.to_string())?;
    fs::write(&json_path, json).map_err(|e| format!("write {}: {}", json_path, e))?;

    Ok(format!("Exported {} and {}", image_path, json_path))
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if let Some(message) = match rerender::maybe_run_cli(&args) {
        Ok(m) => m,
        Err(err) => { eprintln!("{err}"); std::process::exit(1); }
    } { println!("{message}"); return; }
    let targets = load_targets();
    let mut target_index = 0usize;
    let mut evolver = Evolver::new(targets[0].pixels.clone(), DEFAULT_GENES, DEFAULT_MUTATION);

    let view_w = COMP_W as i32 * VIEW_SCALE;
    let view_h = COMP_H as i32 * VIEW_SCALE;
    let panel_x = PAD + view_w + PAD + view_w + PAD;
    let win_w = panel_x + PANEL_W + PAD;
    let win_h = (view_h + PAD * 2).max(760);

    let (mut rl, thread) = raylib::init().size(win_w, win_h).title(TITLE).build();
    rl.set_target_fps(120);

    let mut sim_budget_ms = DEFAULT_BUDGET_MS;
    let mut step_cap = DEFAULT_STEP_CAP;
    let mut paused = false;
    let mut steps_last_frame: usize;
    let mut status_text = String::from("Ready");
    let mut status_timer = 0.0f32;

    while !rl.window_should_close() {
        if rl.is_key_pressed(KeyboardKey::KEY_TAB) {
            target_index = (target_index + 1) % targets.len();
            evolver.set_target(targets[target_index].pixels.clone(), DEFAULT_GENES);
            status_text = format!("Switched target to {}", targets[target_index].name);
            status_timer = 3.0;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_SPACE) {
            paused = !paused;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_R) {
            evolver.reset_random(DEFAULT_GENES);
            status_text = "Reset search state".to_string();
            status_timer = 2.0;
        }

        if rl.is_key_down(KeyboardKey::KEY_UP) {
            evolver.mutation_rate = (evolver.mutation_rate * 1.02).clamp(MIN_MUTATION, MAX_MUTATION);
        }
        if rl.is_key_down(KeyboardKey::KEY_DOWN) {
            evolver.mutation_rate = (evolver.mutation_rate / 1.02).clamp(MIN_MUTATION, MAX_MUTATION);
        }
        if rl.is_key_down(KeyboardKey::KEY_RIGHT) {
            sim_budget_ms = (sim_budget_ms + 0.25).clamp(MIN_BUDGET_MS, MAX_BUDGET_MS);
        }
        if rl.is_key_down(KeyboardKey::KEY_LEFT) {
            sim_budget_ms = (sim_budget_ms - 0.25).clamp(MIN_BUDGET_MS, MAX_BUDGET_MS);
        }

        if rl.is_key_pressed(KeyboardKey::KEY_EQUAL) {
            step_cap = (step_cap.saturating_mul(2)).clamp(MIN_STEP_CAP, MAX_STEP_CAP);
        }
        if rl.is_key_pressed(KeyboardKey::KEY_MINUS) {
            step_cap = (step_cap / 2).clamp(MIN_STEP_CAP, MAX_STEP_CAP);
        }

        let export_rect = Rectangle::new((panel_x + 24) as f32, 420.0, 220.0, 46.0);
        let clicked_export = if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
            point_in_rect(rl.get_mouse_position(), export_rect)
        } else {
            false
        };
        if rl.is_key_pressed(KeyboardKey::KEY_E) || clicked_export {
            match export_best(&evolver, &targets[target_index]) {
                Ok(msg) => {
                    status_text = msg;
                    status_timer = 4.0;
                }
                Err(err) => {
                    status_text = format!("Export failed: {}", err);
                    status_timer = 4.0;
                }
            }
        }

        let sim_start = Instant::now();
        steps_last_frame = 0;
        if !paused {
            let budget = Duration::from_secs_f64(sim_budget_ms / 1000.0);
            while steps_last_frame < step_cap && sim_start.elapsed() < budget {
                evolver.step();
                steps_last_frame += 1;
            }
        }
        status_timer = (status_timer - rl.get_frame_time()).max(0.0);

        let left_x = PAD;
        let top_y = PAD;
        let right_x = left_x + view_w + PAD;

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::new(17, 22, 28, 255));

        draw_rgb_buffer(&mut d, &targets[target_index].pixels, left_x, top_y, VIEW_SCALE, "Target");
        draw_rgb_buffer(&mut d, &evolver.best_pixels, right_x, top_y, VIEW_SCALE, "Best Approx");

        d.draw_rectangle(panel_x, 0, PANEL_W + PAD, win_h, Color::new(24, 31, 40, 255));
        d.draw_text("Triproxim8-rs", panel_x + 24, 24, 34, Color::RAYWHITE);
        d.draw_text(
            &format!("Target: {}", targets[target_index].name),
            panel_x + 24,
            74,
            22,
            Color::new(181, 198, 211, 255),
        );

        d.draw_text("Controls", panel_x + 24, 122, 26, Color::new(230, 240, 250, 255));
        for (i, line) in [
            "UP/DOWN : mutation rate",
            "LEFT/RIGHT : sim budget ms",
            "+/- : step cap",
            "TAB : cycle target",
            "SPACE : pause",
            "R : reset",
            "E or button : export",
        ]
        .iter()
        .enumerate()
        {
            d.draw_text(line, panel_x + 24, 156 + i as i32 * 26, 20, Color::RAYWHITE);
        }

        d.draw_text("Stats", panel_x + 24, 346, 26, Color::new(230, 240, 250, 255));
        let stats = [
            format!("mutation rate : {:.6}", evolver.mutation_rate),
            format!("best loss : {}", evolver.best_loss),
            format!("last loss : {}", evolver.last_loss),
            format!("accepted/rejected : {}/{}", evolver.accepted_steps, evolver.rejected_steps),
            format!("steps total : {}", evolver.total_steps),
            format!("steps/frame : {}", steps_last_frame),
            format!("sim budget ms : {:.2}", sim_budget_ms),
            format!("step cap : {}", step_cap),
            format!("fps : {}", d.get_fps()),
        ];
        for (i, line) in stats.iter().enumerate() {
            d.draw_text(line, panel_x + 24, 378 + i as i32 * 26, 20, Color::RAYWHITE);
        }

        let button_color = if paused {
            Color::new(77, 130, 95, 255)
        } else {
            Color::new(62, 108, 177, 255)
        };
        d.draw_rectangle_rec(export_rect, button_color);
        d.draw_rectangle_lines_ex(export_rect, 2.0, Color::new(215, 225, 235, 255));
        d.draw_text("Export Best (E)", panel_x + 50, 433, 22, Color::WHITE);

        let status_color = if status_timer > 0.0 {
            Color::new(244, 226, 143, 255)
        } else {
            Color::new(136, 150, 164, 255)
        };
        d.draw_text(&status_text, panel_x + 24, 636, 20, status_color);
        if paused {
            d.draw_text("PAUSED", panel_x + 24, 672, 24, Color::new(255, 180, 120, 255));
        }
    }
}
