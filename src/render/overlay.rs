//! Owned UI drawing data; this is independent of VT cells and parser state.
#[derive(Clone, Debug)]
pub struct Rectangle {
    pub rect: [f32; 4],
    pub color: [f32; 4],
}
#[derive(Clone, Debug)]
pub struct Text {
    pub pos: [f32; 2],
    pub width: f32,
    pub value: String,
    pub color: [f32; 4],
}
#[derive(Clone, Debug, Default)]
pub struct Overlay {
    pub rectangles: Vec<Rectangle>,
    pub text: Vec<Text>,
}
