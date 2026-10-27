#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u32); // 0xAARRGGBB

#[allow(dead_code)] // API для будущего рендера и виджетов
impl Color {
    pub const TRANSPARENT: Color = Color(0x00000000);
    pub const BLACK: Color = Color(0xFF000000);
    pub const WHITE: Color = Color(0xFFFFFFFF);

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color(0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    pub fn a(&self) -> u8 {
        ((self.0 >> 24) & 0xFF) as u8
    }
    pub fn r(&self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }
    pub fn g(&self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }
    pub fn b(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }
}