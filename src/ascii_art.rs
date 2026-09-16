//! Converts an arbitrary image into an ASCII-art background that fits
//! exactly into the terminal grid (cols x rows characters).
//!
//! Character set and coloring depend on the detected terminal capabilities
//! (see `caps.rs`):
//! - plain tty console: classic ASCII ramp, monochrome gray
//! - terminal emulator with Unicode: finer ramp with block characters (░▒▓█)
//! - terminal emulator with 256 colors/truecolor: additionally colored in
//!   the image's original colors

use anyhow::{Context, Result};
use image::imageops::FilterType;
use std::path::Path;

/// Classic ASCII brightness ramp, dark -> light (10 levels).
const ASCII_RAMP: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

/// Extended ramp with Unicode block characters for finer gradients,
/// dark -> light (12 levels). Requires a UTF-8 locale.
const UNICODE_RAMP: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '░', '▒', '▓', '█'];

/// A single screen cell: a character plus an optional original color
/// (None = monochrome, rendered in gray by the render side).
#[derive(Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub rgb: Option<(u8, u8, u8)>,
}

/// A fully rasterized text image: `rows` lines of `cols` cells each.
pub struct AsciiFrame {
    pub rows: Vec<Vec<Cell>>,
}

/// Load an image from disk and downscale it to (cols, rows) characters.
///
/// Terminal characters are roughly twice as tall as they are wide, so the
/// sampling height is doubled (two image rows are averaged into one text
/// row) to avoid the image looking squashed.
///
/// `unicode`: use the extended block-character ramp instead of plain ASCII.
/// `colorize`: additionally tint cells with the image's original color
/// (only useful if the terminal supports 256 colors/truecolor).
pub fn from_image<P: AsRef<Path>>(
    path: P,
    cols: u16,
    rows: u16,
    unicode: bool,
    colorize: bool,
) -> Result<AsciiFrame> {
    let img = image::open(path.as_ref())
        .with_context(|| format!("Failed to load image: {:?}", path.as_ref()))?;

    let cols = cols.max(1) as u32;
    let rows = rows.max(1) as u32;
    let ramp: &[char] = if unicode { UNICODE_RAMP } else { ASCII_RAMP };

    // We sample onto cols x (rows*2) and average two pixel rows into one
    // text row to compensate for character height. Resize once in color and
    // derive brightness from that -> saves a second resize pass.
    let sample = img.resize_exact(cols, rows * 2, FilterType::Triangle).to_rgb8();

    let mut out_rows = Vec::with_capacity(rows as usize);
    for ty in 0..rows {
        let mut line = Vec::with_capacity(cols as usize);
        for x in 0..cols {
            let p1 = sample.get_pixel(x, ty * 2);
            let p2 = sample.get_pixel(x, ty * 2 + 1);
            let r = (p1[0] as u16 + p2[0] as u16) / 2;
            let g = (p1[1] as u16 + p2[1] as u16) / 2;
            let b = (p1[2] as u16 + p2[2] as u16) / 2;
            // Standard luminance formula (Rec. 601)
            let lum = ((r * 299 + g * 587 + b * 114) / 1000) as u8;
            let idx = (lum as usize * (ramp.len() - 1)) / 255;
            line.push(Cell {
                ch: ramp[idx],
                rgb: if colorize { Some((r as u8, g as u8, b as u8)) } else { None },
            });
        }
        out_rows.push(line);
    }

    Ok(AsciiFrame { rows: out_rows })
}

/// Fallback background without an image file: a quiet, procedural pattern
/// (a faint diagonal weave) so the desktop doesn't look empty without a wallpaper.
pub fn procedural(cols: u16, rows: u16, unicode: bool) -> AsciiFrame {
    let cols = cols as usize;
    let rows = rows as usize;
    let ascii_chars = [' ', '.', '\'', '`', '.', ' '];
    let unicode_chars = [' ', '.', '\'', '`', '░', '▒', '`', '.', ' '];
    let chars: &[char] = if unicode { &unicode_chars } else { &ascii_chars };

    let mut out_rows = Vec::with_capacity(rows);
    for y in 0..rows {
        let mut line = Vec::with_capacity(cols);
        for x in 0..cols {
            let v = ((x as f32 * 0.15).sin() + (y as f32 * 0.3).sin()) * 0.5 + 0.5;
            let idx = ((v * (chars.len() as f32 - 1.0)) as usize).min(chars.len() - 1);
            line.push(Cell { ch: chars[idx], rgb: None });
        }
        out_rows.push(line);
    }
    AsciiFrame { rows: out_rows }
}
