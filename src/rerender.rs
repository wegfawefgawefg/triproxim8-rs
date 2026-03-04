use image::RgbImage;
use serde::Deserialize;
use std::path::{Path, PathBuf};

const PIXEL_STRIDE: usize = 3;

#[derive(Deserialize)]
struct TriangleConfig {
    triangles: Vec<[f32; 9]>,
}

pub fn maybe_run_cli(args: &[String]) -> Result<Option<String>, String> {
    if !args.iter().any(|a| a == "--rerender") {
        return Ok(None);
    }

    let mut json_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut width: Option<usize> = None;
    let mut height: Option<usize> = None;

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--rerender" => {
                i += 1;
                json_path = Some(PathBuf::from(next_arg(args, i, "--rerender")?));
            }
            "--width" => {
                i += 1;
                width = Some(parse_dim(next_arg(args, i, "--width")?, "--width")?);
            }
            "--height" => {
                i += 1;
                height = Some(parse_dim(next_arg(args, i, "--height")?, "--height")?);
            }
            "--out" => {
                i += 1;
                out_path = Some(PathBuf::from(next_arg(args, i, "--out")?));
            }
            "--help" | "-h" => return Ok(Some(usage_text())),
            _ => {}
        }
        i += 1;
    }

    let json_path = json_path.ok_or_else(|| {
        format!(
            "Missing --rerender <json>.\n{}",
            usage_text()
        )
    })?;
    let width = width.ok_or_else(|| format!("Missing --width <pixels>.\n{}", usage_text()))?;
    let height = height.ok_or_else(|| format!("Missing --height <pixels>.\n{}", usage_text()))?;
    let out_path = out_path.unwrap_or_else(|| default_output_path(&json_path, width, height));

    rerender_json_to_png(&json_path, width, height, &out_path)
        .map(|triangles| Some(format!(
            "Rerendered {} triangles to {} ({}x{})",
            triangles,
            out_path.display(),
            width,
            height
        )))
}

fn next_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(|s| s.as_str())
        .ok_or_else(|| format!("Expected value after {}", flag))
}

fn parse_dim(raw: &str, field: &str) -> Result<usize, String> {
    let val = raw
        .parse::<usize>()
        .map_err(|_| format!("{} expects an integer, got '{}'", field, raw))?;
    if val == 0 {
        return Err(format!("{} must be > 0", field));
    }
    if val > 32_768 {
        return Err(format!("{} too large (max 32768)", field));
    }
    Ok(val)
}

fn usage_text() -> String {
    "Usage: cargo run --release -- --rerender <export.json> --width <pixels> --height <pixels> [--out <output.png>]".to_string()
}

fn default_output_path(json_path: &Path, width: usize, height: usize) -> PathBuf {
    let stem = json_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("rerender");
    PathBuf::from(format!("exports/{}_{}x{}.png", stem, width, height))
}

fn rerender_json_to_png(
    json_path: &Path,
    width: usize,
    height: usize,
    out_path: &Path,
) -> Result<usize, String> {
    let body = std::fs::read_to_string(json_path)
        .map_err(|e| format!("read {}: {}", json_path.display(), e))?;
    let cfg: TriangleConfig = serde_json::from_str(&body)
        .map_err(|e| format!("parse {}: {}", json_path.display(), e))?;

    if cfg.triangles.is_empty() {
        return Err(format!("{} contains no triangles", json_path.display()));
    }

    let mut pixels = vec![0u8; width * height * PIXEL_STRIDE];
    for tri in &cfg.triangles {
        rasterize_triangle(&mut pixels, width, height, tri);
    }

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {}", parent.display(), e))?;
        }
    }

    RgbImage::from_raw(width as u32, height as u32, pixels)
        .ok_or_else(|| "Failed to construct output image buffer".to_string())?
        .save(out_path)
        .map_err(|e| format!("save {}: {}", out_path.display(), e))?;

    Ok(cfg.triangles.len())
}

fn rasterize_triangle(out: &mut [u8], width: usize, height: usize, tri: &[f32; 9]) {
    let x1 = tri[0].clamp(0.0, 1.0) * width as f32;
    let y1 = tri[1].clamp(0.0, 1.0) * height as f32;
    let x2 = tri[2].clamp(0.0, 1.0) * width as f32;
    let y2 = tri[3].clamp(0.0, 1.0) * height as f32;
    let x3 = tri[4].clamp(0.0, 1.0) * width as f32;
    let y3 = tri[5].clamp(0.0, 1.0) * height as f32;

    let r = (tri[6].clamp(0.0, 1.0) * 255.0) as u8;
    let g = (tri[7].clamp(0.0, 1.0) * 255.0) as u8;
    let b = (tri[8].clamp(0.0, 1.0) * 255.0) as u8;

    let min_x = x1.min(x2).min(x3).floor().max(0.0) as i32;
    let min_y = y1.min(y2).min(y3).floor().max(0.0) as i32;
    let max_x = x1.max(x2).max(x3).ceil().min((width - 1) as f32) as i32;
    let max_y = y1.max(y2).max(y3).ceil().min((height - 1) as f32) as i32;
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
            let idx = ((py as usize * width) + px as usize) * PIXEL_STRIDE;
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
