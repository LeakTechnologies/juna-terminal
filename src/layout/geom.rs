#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> i64 {
        self.x as i64 + self.w as i64
    }

    pub fn bottom(&self) -> i64 {
        self.y as i64 + self.h as i64
    }

    pub fn area(&self) -> i128 {
        self.w as i128 * self.h as i128
    }

    pub fn contains(&self, other: &Rect) -> bool {
        (other.x as i64) >= (self.x as i64)
            && (other.y as i64) >= (self.y as i64)
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    pub fn overlaps(&self, other: &Rect) -> bool {
        (self.x as i64) < other.right()
            && (other.x as i64) < self.right()
            && (self.y as i64) < other.bottom()
            && (other.y as i64) < self.bottom()
    }

    pub fn center(&self) -> (i64, i64) {
        (self.x as i64 * 2 + self.w as i64, self.y as i64 * 2 + self.h as i64)
    }

    pub fn split_h(&self, permille: u16) -> (Rect, Rect) {
        let total = self.w as i64;
        let mut first = div_permille(total, permille);
        first = first.clamp(0, total);
        (
            Rect::new(self.x, self.y, first as u32, self.h),
            Rect::new(
                offset(self.x, first),
                self.y,
                (total - first) as u32,
                self.h,
            ),
        )
    }

    pub fn split_v(&self, permille: u16) -> (Rect, Rect) {
        let total = self.h as i64;
        let mut first = div_permille(total, permille);
        first = first.clamp(0, total);
(
            Rect::new(self.x, self.y, self.w, first as u32),
            Rect::new(
                self.x,
                offset(self.y, first),
                self.w,
                (total - first) as u32,
            ),
        )
    }
}

pub(crate) fn div_permille(total: i64, permille: u16) -> i64 {
    if total <= 0 {
        return 0;
    }
    total * permille as i64 / 1000
}

pub(crate) fn offset(base: i32, delta: i64) -> i32 {
    (base as i64 + delta).clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_and_bottom_use_wide_arithmetic() {
        let r = Rect::new(i32::MAX - 1, 0, 10, 10);
        assert_eq!(r.right(), i32::MAX as i64 + 9);
    }

    #[test]
    fn area_does_not_overflow_on_extreme_rects() {
        let r = Rect::new(0, 0, u32::MAX, u32::MAX);
        assert_eq!(
            r.area(),
            (u32::MAX as i128) * (u32::MAX as i128),
            "u32::MAX squared must not overflow"
        );
    }

    #[test]
    fn horizontal_split_conserves_width_and_position() {
        let r = Rect::new(10, 20, 101, 30);
        let (a, b) = r.split_h(500);
        assert_eq!(a.w + b.w, r.w);
        assert_eq!(b.x, a.right() as i32);
        assert!(r.contains(&a) && r.contains(&b));
    }

    #[test]
    fn split_rounding_is_deterministic_and_exact() {
        let r = Rect::new(0, 0, 101, 101);
        let (a, _) = r.split_h(333);
        assert_eq!(a.w, 33);
        for _ in 0..64 {
            let (again, _) = r.split_h(333);
            assert_eq!(again, a);
        }
    }

    #[test]
    fn split_of_zero_extent_yields_zero_children() {
        let r = Rect::new(5, 5, 0, 0);
        let (a, b) = r.split_h(500);
        assert_eq!(a, Rect::new(5, 5, 0, 0));
        assert_eq!(b, Rect::new(5, 5, 0, 0));
    }

    #[test]
    fn permille_division_matches_integer_contract() {
        assert_eq!(div_permille(1000, 500), 500);
        assert_eq!(div_permille(1, 999), 0);
        assert_eq!(div_permille(-5, 500), 0);
    }
}