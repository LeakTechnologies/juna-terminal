//! Pure presentation geometry; row numbers are not terminal cells.
use crate::render::overlay::{Overlay, Text};
const GAP_CELLS: usize = 2;
pub fn width(viewport: f32, scale: f32, enabled: bool) -> f32 {
    if !enabled || scale <= 0. {
        return 0.;
    }
    let slots = ((viewport / scale - 16.) / 10.).floor().max(0.) as usize;
    let mut digits = 3;
    for _ in 0..8 {
        if slots < digits + GAP_CELLS + 1 {
            return 0.;
        }
        let cols = (slots - digits - GAP_CELLS).clamp(1, 512);
        let capacity =
            crate::vt::scrollback::DEFAULT_BUDGET / (cols * std::mem::size_of::<crate::vt::Cell>());
        let needed = (capacity + 512).to_string().len();
        if needed <= digits {
            break;
        }
        digits = needed;
    }
    (digits + GAP_CELLS) as f32 * 10. * scale
}
pub fn scene(inset: f32, scale: f32, first: usize, rows: u16, color: [f32; 4]) -> Overlay {
    let mut out = Overlay::default();
    if inset <= 0. || !inset.is_finite() || !scale.is_finite() || scale <= 0. {
        return out;
    }
    let digits = ((inset / (10. * scale)).round() as usize)
        .saturating_sub(GAP_CELLS)
        .clamp(1, 8);
    for row in 0..rows {
        out.text.push(Text {
            pos: [
                super::PAD * scale,
                (super::PAD + super::ui::TAB_HEIGHT + row as f32 * super::CELL_H) * scale,
            ],
            width: inset - GAP_CELLS as f32 * 10. * scale,
            value: format!(
                "{:>digits$}",
                first.saturating_add(row as usize).saturating_add(1)
            ),
            color,
        });
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_geometry_and_retained_row_numbers() {
        assert_eq!(width(1024., 1., false), 0.);
        assert_eq!(width(40., 1., true), 0.);
        let w = width(1024., 1., true);
        assert_eq!(width(2048., 2., true), w * 2.);
        let s = scene(w, 1., 171, 3, [1.; 4]);
        assert_eq!(s.text[0].value.trim(), "172");
        assert_eq!(s.text[2].value.trim(), "174");
        assert_eq!(s.text[1].pos[1] - s.text[0].pos[1], super::super::CELL_H);
    }
}
