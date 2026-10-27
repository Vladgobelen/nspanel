/// Режим размера виджета вдоль панели.
///
/// Панель фиксирована по толщине: высота для top/bottom, ширина для
/// left/right. Виджет не может быть произвольным поперёк панели —
/// его ограничивает `panel_thickness - 2*margin`.
///
/// - `Fixed { length, margin }` — длина вдоль панели задаётся виджетом,
///   поперёк — всегда `panel_thickness - 2*margin` (задаёт панель).
/// - `Stretch { margin, ratio }` — поперёк тоже `panel_thickness - 2*margin`,
///   а вдоль — `поперёк * ratio` (для горизонтали), либо `поперёк`
///   (для вертикали).
#[derive(Debug, Clone, Copy)]
pub enum SizeMode {
    Fixed { length: u32, margin: u32 },
    Stretch { margin: u32, ratio: f32 },
}

impl SizeMode {
    /// Размер виджета `(ширина, высота)` в пикселях.
    ///
    /// `panel_thickness` — толщина панели (высота для top/bottom,
    /// ширина для left/right). `vertical` — панель вертикальная.
    ///
    /// Поперёк панели всегда `panel_thickness - 2*margin`.
    /// Вдоль панели:
    /// - `Fixed` — `length`;
    /// - `Stretch` — `поперёк * ratio` для горизонтали, `поперёк`
    ///   для вертикали.
    pub fn preferred_size(&self, panel_thickness: u32, vertical: bool) -> (u32, u32) {
        match *self {
            SizeMode::Fixed { length, margin } => {
                let across = panel_thickness.saturating_sub(2 * margin);
                if vertical {
                    (across, length)
                } else {
                    (length, across)
                }
            }
            SizeMode::Stretch { margin, ratio } => {
                let across = panel_thickness.saturating_sub(2 * margin);
                let along = if vertical {
                    across
                } else {
                    ((across as f32) * ratio).max(0.0) as u32
                };
                if vertical {
                    (across, along)
                } else {
                    (along, across)
                }
            }
        }
    }
}