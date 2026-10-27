use std::sync::{Arc, Mutex};

use crate::core::{Color, Rect};
use crate::render::Font;

pub type SharedBuffer = Arc<Mutex<PixelBuffer>>;

pub struct PixelBuffer {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u32>, // 0xAARRGGBB
}

impl PixelBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; (width * height) as usize],
        }
    }

    pub fn new_shared(width: u32, height: u32) -> SharedBuffer {
        Arc::new(Mutex::new(Self::new(width, height)))
    }

    pub fn fill_rect(&mut self, area: Rect, color: Color) {
        let x0 = area.x.max(0) as u32;
        let y0 = area.y.max(0) as u32;
        let x1 = (area.right().max(0) as u32).min(self.width);
        let y1 = (area.bottom().max(0) as u32).min(self.height);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for y in y0..y1 {
            let row = (y * self.width) as usize;
            for x in x0..x1 {
                self.data[row + x as usize] = color.0;
            }
        }
    }

    pub fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: Color) {
        let dx = (x2 - x1).abs();
        let dy = -(y2 - y1).abs();
        let sx = if x1 < x2 { 1 } else { -1 };
        let sy = if y1 < y2 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut x = x1;
        let mut y = y1;
        loop {
            self.put_pixel(x, y, color);
            if x == x2 && y == y2 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    pub fn draw_argb_pixels(&mut self, x: i32, y: i32, w: i32, h: i32, pixels: &[u32]) {
        if w <= 0 || h <= 0 {
            return;
        }
        let expected = (w * h) as usize;
        if pixels.len() < expected {
            return;
        }

        for dy in 0..h {
            for dx in 0..w {
                let px = pixels[(dy * w + dx) as usize];
                let a = (px >> 24) & 0xFF;
                if a == 0 {
                    continue;
                }
                let color = Color(px);
                self.put_pixel(x + dx, y + dy, color);
            }
        }
    }

    fn put_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let idx = (y as u32 * self.width + x as u32) as usize;
        let a = color.a();
        if a == 255 {
            self.data[idx] = color.0;
        } else if a > 0 {
            let dst = self.data[idx];
            let da = ((dst >> 24) & 0xFF) as u32;
            let dr = (dst >> 16) & 0xFF;
            let dg = (dst >> 8) & 0xFF;
            let db = dst & 0xFF;
            let sr = color.r() as u32;
            let sg = color.g() as u32;
            let sb = color.b() as u32;
            let aa = a as u32;
            let out_a = aa + da * (255 - aa) / 255;
            let out_r = (sr * aa + dr * (255 - aa)) / 255;
            let out_g = (sg * aa + dg * (255 - aa)) / 255;
            let out_b = (sb * aa + db * (255 - aa)) / 255;
            self.data[idx] = (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
        }
    }

    pub fn draw_text(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        size: f32,
        font: &Font,
    ) {
        let mut cursor_x = x;
        for ch in text.chars() {
            let (metrics, coverage) = font.rasterize(ch, size);
            let gx = cursor_x + metrics.xmin;
            let gy = y + (size as i32 - metrics.height as i32) - metrics.ymin;
            for cy in 0..metrics.height {
                for cx in 0..metrics.width {
                    let cov = coverage[cy * metrics.width + cx];
                    if cov == 0 {
                        continue;
                    }
                    let px = gx + cx as i32;
                    let py = gy + cy as i32;
                    let alpha = (cov as u32 * color.a() as u32) / 255;
                    if alpha == 0 {
                        continue;
                    }
                    let c = Color((alpha << 24) | (color.0 & 0x00FFFFFF));
                    self.put_pixel(px, py, c);
                }
            }
            cursor_x += metrics.advance_width as i32;
        }
    }
}