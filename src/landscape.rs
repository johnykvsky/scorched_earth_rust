//! Reproducible landscape recipes and bounded external mountain decoders.
use crate::gfx::{Rgb, H, W};
use crate::settings::{LandscapeSettings, Sky};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

const MAX_FILE: usize = 16 * 1024 * 1024;
const MAX_PIXELS: usize = 4 * 1024 * 1024;

pub struct Random(u64);
impl Random {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn range(&mut self, max: u32) -> u32 {
        (self.next() % u64::from(max.max(1))) as u32
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 40) as f64 / 16_777_216.0
    }
}

pub const RANDOM_SKIES: [Sky; 6] = [
    Sky::Plain,
    Sky::Stormy,
    Sky::Stars,
    Sky::Shaded,
    Sky::Sunset,
    Sky::Cavern,
];
pub fn choose_sky(sky: Sky, rng: &mut Random) -> Sky {
    if sky == Sky::Random {
        RANDOM_SKIES[rng.range(6) as usize]
    } else {
        sky
    }
}

/// Integer random walk selected by DefaultLand's vtable in v1.5.
/// The midpoint-displacement routine elsewhere in SCORCH.EXE is not selected.
/// See tools/re/terrain_oracle.py and TERRAIN_FOLLOWUP.md for the proof chain.
pub fn walk(
    rng: &mut Random,
    width: usize,
    top: i32,
    bottom: i32,
    bumpiness: u8,
    slope: u8,
    flatten: bool,
    reverse: bool,
) -> Vec<i32> {
    assert!(width > 0 && bottom - top > 60);
    let mut y = rng.range((bottom - top - 60) as u32) as i32 + top + 40;
    let mut velocity = 0;
    let mut heights = vec![0; width];
    for step in 0..width {
        let x = if reverse { width - 1 - step } else { step };
        heights[x] = y;
        if rng.range(100) < u32::from(bumpiness) {
            let mut change = rng.range(3) as i32 - 1;
            if rng.range(100) < u32::from(slope) {
                change *= 2;
            }
            velocity += change;
            if flatten && !(-3..=3).contains(&velocity) {
                velocity = 0;
            }
        }
        y += velocity;
        if y < top + 40 {
            y = top + 40;
            if velocity < 0 {
                velocity = 1;
            }
        }
        if y > bottom - 1 {
            y = bottom - 1;
            if velocity > 0 {
                velocity = 0;
            }
        }
    }
    heights
}

pub struct Contours {
    pub floor: Vec<i32>,
    pub ceiling: Vec<i32>,
}

pub fn contours(
    settings: &LandscapeSettings,
    seed: u64,
    top: i32,
    cavern: bool,
    reverse: bool,
) -> Contours {
    let mut rng = Random::new(seed);
    let (bump, slope, flat) = if settings.classic_terrain {
        (20, 20, false)
    } else {
        (settings.bumpiness, settings.slope, settings.flatten_peaks)
    };
    let bottom = H - 1;
    let middle = (top + bottom) / 2;
    let floor = walk(
        &mut rng,
        W as usize,
        if cavern { middle } else { top },
        bottom,
        bump,
        slope,
        flat,
        reverse,
    );
    let ceiling = if cavern {
        walk(
            &mut rng, W as usize, top, middle, bump, slope, flat, reverse,
        )
    } else {
        vec![top; W as usize]
    };
    Contours { floor, ceiling }
}

pub struct Mountain {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Option<Rgb>>,
}
fn dimensions(width: usize, height: usize) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || width
            .checked_mul(height)
            .is_none_or(|count| count > MAX_PIXELS)
    {
        Err("Mountain dimensions exceed the 4096-side / 4M-pixel limit".into())
    } else {
        Ok(())
    }
}
pub fn decode_mtn(bytes: &[u8]) -> Result<Mountain, String> {
    if bytes.len() < 24
        || bytes[..4] != [0x4d, 0x54, 0xbe, 0xef]
        || u16::from_be_bytes([bytes[4], bytes[5]]) != 1
    {
        return Err("Invalid MTN header or version".into());
    }
    let word = |at| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
    let (width, height, colors) = (word(6), word(10), word(12));
    dimensions(width, height)?;
    if word(8) > height || !(1..=16).contains(&colors) || word(16) >= colors {
        return Err("Invalid MTN palette or heights".into());
    }
    let mut at = 24;
    let palette_bytes = bytes
        .get(at..at + colors * 3)
        .ok_or("Truncated MTN palette")?;
    let palette: Vec<Rgb> = palette_bytes
        .chunks_exact(3)
        .map(|c| Rgb(c[0], c[1], c[2]))
        .collect();
    at += colors * 3;
    let mut pixels = vec![None; width * height];
    for x in 0..width {
        let count_bytes = bytes.get(at..at + 2).ok_or("Truncated MTN column header")?;
        let count = u16::from_le_bytes([count_bytes[0], count_bytes[1]]) as usize;
        at += 2;
        if count > height {
            return Err("MTN column exceeds image height".into());
        }
        let column = bytes
            .get(at..at + count.div_ceil(2))
            .ok_or("Truncated MTN pixels")?;
        for y in 0..count {
            let index = ((column[y / 2] >> ((y % 2) * 4)) & 15) as usize;
            pixels[(height - 1 - y) * width + x] = Some(
                *palette
                    .get(index)
                    .ok_or("MTN pixel is outside the palette")?,
            );
        }
        at += count.div_ceil(2);
    }
    if at != bytes.len() {
        return Err("Unexpected data after MTN columns".into());
    }
    if pixels.iter().all(Option::is_none) {
        return Err("Mountain contains no ground".into());
    }
    Ok(Mountain {
        width,
        height,
        pixels,
    })
}

pub fn decode_png(bytes: &[u8]) -> Result<Mountain, String> {
    if bytes.len() < 33 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return Err("Invalid PNG header".into());
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap()) as usize;
    dimensions(width, height)?;
    let image = macroquad::prelude::Image::from_file_with_format(bytes, None)
        .map_err(|error| format!("Invalid PNG: {error}"))?;
    if usize::from(image.width) != width || usize::from(image.height) != height {
        return Err("PNG dimensions changed during decoding".into());
    }
    let alpha_sky = image.bytes.chunks_exact(4).any(|c| c[3] < 128);
    let mut pixels: Vec<Option<Rgb>> = image
        .bytes
        .chunks_exact(4)
        .map(|c| {
            if c[3] < 128 {
                None
            } else {
                Some(Rgb(c[0], c[1], c[2]))
            }
        })
        .collect();
    if !alpha_sky {
        // Flat top-left background colour is a convenience for opaque PNGs.
        // Flood only top-connected pixels so matching spots in rocks survive.
        let key = pixels[0];
        let mut queue = Vec::new();
        for x in 0..width {
            if pixels[x] == key {
                pixels[x] = None;
                queue.push(x);
            }
        }
        while let Some(i) = queue.pop() {
            let x = i % width;
            let y = i / width;
            let neighbors = [
                if x > 0 { Some(i - 1) } else { None },
                if x + 1 < width { Some(i + 1) } else { None },
                if y > 0 { Some(i - width) } else { None },
                if y + 1 < height {
                    Some(i + width)
                } else {
                    None
                },
            ];
            for next in neighbors.into_iter().flatten() {
                if pixels[next] == key {
                    pixels[next] = None;
                    queue.push(next);
                }
            }
        }
    }
    if pixels.iter().all(Option::is_none) {
        return Err("PNG mountain contains no ground".into());
    }
    Ok(Mountain {
        width,
        height,
        pixels,
    })
}

fn read_mountain(path: &Path) -> Result<Mountain, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take((MAX_FILE + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE {
        return Err("Mountain exceeds the 16 MiB file limit".into());
    }
    if path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("mtn"))
    {
        decode_mtn(&bytes)
    } else {
        decode_png(&bytes)
    }
}
pub fn load_mountain(folder: &Path, rng: &mut Random) -> Result<Mountain, String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(folder)
        .map_err(|e| format!("Cannot read mountains folder: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("mtn") || s.eq_ignore_ascii_case("png"))
        })
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err("Mountains folder contains no MTN or PNG files".into());
    }
    let start = rng.range(paths.len() as u32) as usize;
    for offset in 0..paths.len() {
        if let Ok(mountain) = read_mountain(&paths[(start + offset) % paths.len()]) {
            return Ok(mountain);
        }
    }
    Err("No readable mountain in the folder (check format and size limits)".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    pub fn mtn_fixture() -> Vec<u8> {
        let mut bytes = vec![0; 24];
        bytes[..6].copy_from_slice(&[0x4d, 0x54, 0xbe, 0xef, 0, 1]);
        bytes[6..8].copy_from_slice(&2_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&3_u16.to_le_bytes());
        bytes[12..14].copy_from_slice(&3_u16.to_le_bytes());
        bytes.extend_from_slice(&[255, 255, 255, 100, 40, 20, 200, 90, 30]);
        bytes.extend_from_slice(&[3, 0, 0x21, 0x01, 1, 0, 0x02]);
        bytes
    }
    #[test]
    fn random_walk_matches_original_instruction_oracle() {
        #[derive(serde::Deserialize)]
        struct Oracle {
            exe_sha256: String,
            case: Vec<Case>,
        }
        #[derive(serde::Deserialize)]
        struct Case {
            seed: u64,
            width: usize,
            top: i32,
            bottom: i32,
            bump: u8,
            slope: u8,
            flat: bool,
            reverse: bool,
            cavern: bool,
            floor: Vec<i32>,
            ceiling: Vec<i32>,
        }
        let oracle: Oracle =
            toml::from_str(include_str!("../tests/fixtures/terrain-oracle.toml")).unwrap();
        assert_eq!(
            oracle.exe_sha256,
            "05e4a11643ec7055b1c811531a73c5b4f5c67c463612f3e4910e557301c3179f"
        );
        for case in oracle.case {
            let mut rng = Random::new(case.seed);
            let middle = (case.top + case.bottom) / 2;
            let floor = walk(
                &mut rng,
                case.width,
                if case.cavern { middle } else { case.top },
                case.bottom,
                case.bump,
                case.slope,
                case.flat,
                case.reverse,
            );
            assert_eq!(floor, case.floor, "floor seed {}", case.seed);
            if case.cavern {
                let ceiling = walk(
                    &mut rng,
                    case.width,
                    case.top,
                    middle,
                    case.bump,
                    case.slope,
                    case.flat,
                    case.reverse,
                );
                assert_eq!(ceiling, case.ceiling, "ceiling seed {}", case.seed);
            }
        }
    }

    #[test]
    fn classic_defaults_ignore_sliders_and_walk_ends_are_independent() {
        let settings = Settings::default().landscape;
        let mut unequal = 0;
        for seed in 1..=120 {
            let seed = seed * 7919;
            let default = contours(&settings, seed, 22, false, false);
            unequal += usize::from(default.floor[0] != default.floor[W as usize - 1]);
            let mut classic = settings.clone();
            classic.classic_terrain = true;
            classic.bumpiness = 99;
            classic.slope = 0;
            classic.flatten_peaks = true;
            assert_eq!(
                default.floor,
                contours(&classic, seed, 22, false, false).floor
            );
            let mut backwards = contours(&settings, seed, 22, false, true).floor;
            backwards.reverse();
            assert_eq!(default.floor, backwards);
        }
        assert!(unequal > 100);
        let mut settings = settings;
        settings.bumpiness = 0;
        assert!(contours(&settings, 456789, 22, false, false)
            .floor
            .windows(2)
            .all(|p| p[0] == p[1]));
        settings.bumpiness = 100;
        settings.slope = 0;
        let gentle = contours(&settings, 456789, 22, false, false).floor;
        settings.slope = 100;
        let steep = contours(&settings, 456789, 22, false, false).floor;
        assert_ne!(gentle, steep);
        settings.flatten_peaks = true;
        let flat = contours(&settings, 456789, 22, false, false).floor;
        assert_ne!(flat, steep);
        assert!(flat.windows(2).all(|p| (p[0] - p[1]).abs() <= 3));
    }

    #[test]
    fn native_caverns_use_independent_walks_in_separate_vertical_halves() {
        for top in [22, 44, 120, 320] {
            let middle = (top + H - 1) / 2;
            for seed in 1..=60 {
                let settings = Settings::default().landscape;
                let c = contours(&settings, seed * 7919, top, true, false);
                assert!(c.floor.iter().all(|&y| (middle + 40..=H - 2).contains(&y)));
                assert!(c.ceiling.iter().all(|&y| (top + 40..middle).contains(&y)));
                assert!(c
                    .floor
                    .iter()
                    .zip(&c.ceiling)
                    .all(|(floor, ceiling)| floor - ceiling >= 41));
                assert_ne!(c.floor.iter().map(|y| H - y).collect::<Vec<_>>(), c.ceiling);
            }
        }
    }
    #[test]
    fn mtn_decodes_odd_columns_bottom_up_and_rejects_invalid_data() {
        let bytes = mtn_fixture();
        let mountain = decode_mtn(&bytes).unwrap();
        assert_eq!((mountain.width, mountain.height), (2, 3));
        assert_eq!(
            mountain.pixels,
            vec![
                Some(Rgb(100, 40, 20)),
                None,
                Some(Rgb(200, 90, 30)),
                None,
                Some(Rgb(100, 40, 20)),
                Some(Rgb(200, 90, 30))
            ]
        );
        for length in 0..bytes.len() {
            assert!(decode_mtn(&bytes[..length]).is_err());
        }
        for (index, value) in [(0, 0), (5, 2), (12, 17), (16, 3), (33, 4), (35, 15)] {
            let mut invalid = bytes.clone();
            invalid[index] = value;
            assert!(decode_mtn(&invalid).is_err(), "index {index}");
        }
        let mut extra = bytes;
        extra.push(0);
        assert!(decode_mtn(&extra).is_err());
    }
    #[test]
    fn png_respects_alpha_and_opaque_top_connected_background() {
        let transparent =
            decode_png(include_bytes!("../tests/fixtures/mountain-alpha.png")).unwrap();
        assert_eq!((transparent.width, transparent.height), (3, 3));
        assert!(transparent.pixels[..3].iter().all(Option::is_none));
        assert_eq!(transparent.pixels[4], Some(Rgb(120, 60, 20)));
        let opaque = decode_png(include_bytes!("../tests/fixtures/mountain-opaque.png")).unwrap();
        assert!(opaque.pixels[..3].iter().all(Option::is_none));
        assert_eq!(opaque.pixels[7], Some(Rgb(0, 0, 100))); // enclosed sky-coloured rock
        assert!(decode_png(b"broken").is_err());
        let mut huge = include_bytes!("../tests/fixtures/mountain-alpha.png").to_vec();
        huge[16..20].copy_from_slice(&5000_u32.to_be_bytes());
        assert!(decode_png(&huge).is_err());
    }
    #[test]
    fn external_selection_skips_bad_files_and_percent_zero_never_loads() {
        let folder =
            std::env::temp_dir().join(format!("scorched-mountains-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("broken.MTN"), b"bad").unwrap();
        std::fs::write(folder.join("valid.MTN"), mtn_fixture()).unwrap();
        assert!(load_mountain(&folder, &mut Random::new(7)).is_ok());
        let mut settings = Settings::default().landscape;
        settings.mountains_folder = Some(folder.clone());
        settings.scanned_mountains_percent = 100;
        let (terrain, notice) = crate::terrain::Terrain::generate_for(&settings, 22);
        assert!(terrain.scanned && notice.is_none());
        settings.mountains_folder = Some(folder.join("missing"));
        settings.scanned_mountains_percent = 0;
        let (terrain, notice) = crate::terrain::Terrain::generate_for(&settings, 22);
        assert!(!terrain.scanned && notice.is_none());
        settings.scanned_mountains_percent = 100;
        let (terrain, notice) = crate::terrain::Terrain::generate_for(&settings, 22);
        assert!(!terrain.scanned && notice.is_some());
        std::fs::remove_dir_all(folder).unwrap();
    }
}
