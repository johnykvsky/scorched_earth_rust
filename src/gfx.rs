//! Software framebuffer: everything is drawn pixel by pixel into a 640x480
//! image, which is then scaled up with nearest-neighbour filtering.

use font8x8::{UnicodeFonts, BASIC_FONTS};
use macroquad::prelude::{Image, BLACK};

pub const W: i32 = 640;
pub const H: i32 = 480;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const BLACK: Rgb = Rgb(0, 0, 0);
    pub const WHITE: Rgb = Rgb(255, 255, 255);
    pub const GRAY: Rgb = Rgb(170, 170, 170);
    pub const DARK: Rgb = Rgb(85, 85, 85);

    pub fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
        let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
        Rgb(f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
    }

    pub fn scale(self, k: f32) -> Rgb {
        let f = |x: u8| (x as f32 * k).min(255.0) as u8;
        Rgb(f(self.0), f(self.1), f(self.2))
    }
}

pub struct Fb {
    pub img: Image,
}

impl Fb {
    pub fn new() -> Self {
        Fb {
            img: Image::gen_image_color(W as u16, H as u16, BLACK),
        }
    }

    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.img.bytes
    }

    #[inline]
    pub fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if x < 0 || y < 0 || x >= W || y >= H {
            return;
        }
        let i = ((y * W + x) * 4) as usize;
        let p = &mut self.img.bytes[i..i + 4];
        p[0] = c.0;
        p[1] = c.1;
        p[2] = c.2;
        p[3] = 255;
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, c);
            }
        }
    }

    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        self.rect(x, y, w, 1, c);
        self.rect(x, y + h - 1, w, 1, c);
        self.rect(x, y, 1, h, c);
        self.rect(x + w - 1, y, 1, h, c);
    }

    /// Classic grey 3D panel.
    pub fn bevel(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.rect(x, y, w, h, Rgb::GRAY);
        for i in 0..2 {
            self.rect(x + i, y + i, w - 2 * i, 1, Rgb::WHITE);
            self.rect(x + i, y + i, 1, h - 2 * i, Rgb::WHITE);
            self.rect(x + i, y + h - 1 - i, w - 2 * i, 1, Rgb::DARK);
            self.rect(x + w - 1 - i, y + i, 1, h - 2 * i, Rgb::DARK);
        }
    }

    pub fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, c: Rgb) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.put(x0, y0, c);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Text in the DOS 8x16 look (8x8 glyphs, doubled vertically) times `s`.
    pub fn text_scaled(&mut self, x: i32, y: i32, s: &str, c: Rgb, s_x: i32, s_y: i32) {
        for (n, ch) in s.chars().enumerate() {
            let Some(glyph) = BASIC_FONTS.get(ch) else {
                continue;
            };
            let gx = x + n as i32 * 8 * s_x;
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..8 {
                    if bits & (1 << col) != 0 {
                        self.rect(gx + col * s_x, y + row as i32 * 2 * s_y, s_x, 2 * s_y, c);
                    }
                }
            }
        }
    }

    pub fn text(&mut self, x: i32, y: i32, s: &str, c: Rgb) {
        self.text_scaled(x, y, s, c, 1, 1);
    }

    pub fn text_centered(&mut self, y: i32, s: &str, c: Rgb) {
        self.text(W / 2 - text_w(s) / 2, y, s, c);
    }
}

pub fn text_w(s: &str) -> i32 {
    s.chars().count() as i32 * 8
}

/// Shared by drawing and mouse hit testing, including integer scale and letterboxing.
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

impl Viewport {
    pub fn new(width: f32, height: f32) -> Self {
        let fit = (width / W as f32).min(height / H as f32).max(0.001);
        let scale = if fit >= 1.0 { fit.floor() } else { fit };
        let (w, h) = (W as f32 * scale, H as f32 * scale);
        Self {
            x: ((width - w) / 2.0).floor(),
            y: ((height - h) / 2.0).floor(),
            width: w,
            height: h,
            scale,
        }
    }

    pub fn point(&self, x: f32, y: f32) -> (i32, i32) {
        (
            ((x - self.x) / self.scale).floor() as i32,
            ((y - self.y) / self.scale).floor() as i32,
        )
    }
}

#[cfg(test)]
mod viewport_tests {
    use super::*;
    #[test]
    fn mouse_coordinates_follow_integer_and_fractional_scaling_and_letterboxing() {
        for (width, height) in [(1280.0, 960.0), (1500.0, 1000.0), (320.0, 240.0)] {
            let view = Viewport::new(width, height);
            assert_eq!(
                view.point(view.x + 100.0 * view.scale, view.y + 200.0 * view.scale),
                (100, 200)
            );
            assert_eq!(view.point(view.x - 1.0, view.y - 1.0).0 < 0, true);
            assert_eq!(
                view.point(view.x + view.width, view.y + view.height),
                (W, H)
            );
        }
    }
}

/// A round's persistent paths occupy at most one colour per framebuffer pixel.
pub struct TraceLayer {
    pixels: Vec<Option<Rgb>>,
}
impl Default for TraceLayer {
    fn default() -> Self {
        Self {
            pixels: vec![None; (W * H) as usize],
        }
    }
}
impl TraceLayer {
    pub fn clear(&mut self) {
        self.pixels.fill(None);
    }
    pub fn record(&mut self, points: &[(i32, i32, Rgb)], owner_color: Option<Rgb>) {
        for &(x, y, color) in points {
            if (0..W).contains(&x) && (0..H).contains(&y) {
                self.pixels[(y * W + x) as usize] = Some(owner_color.unwrap_or(color));
            }
        }
    }
    pub fn draw(&self, fb: &mut Fb) {
        for (i, color) in self.pixels.iter().enumerate() {
            if let Some(color) = color {
                fb.put(i as i32 % W, i as i32 / W, *color);
            }
        }
    }
    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.pixels.iter().filter(|pixel| pixel.is_some()).count()
    }
}
