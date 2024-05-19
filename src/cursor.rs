use std::num::NonZeroUsize;

pub trait Cursor: Iterator {
    fn advance_by(&mut self, n: usize) -> Result<(), NonZeroUsize>;
    fn prev(&mut self) -> Option<Self::Item>;
    fn current(&self) -> Option<Self::Item>;
    fn peek_next(&self) -> Option<Self::Item>;
    fn peek_prev(&self) -> Option<Self::Item>;
    fn revert_by(&mut self, n: usize) -> Result<(), NonZeroUsize>;
    fn at_start(&self) -> bool;
    fn at_end(&self) -> bool;
}

pub enum CursorPosition {
    Start,
    Pos(usize),
    End,
}

pub trait CursorWithPosition: Cursor {
    fn pos(&self) -> CursorPosition;
    fn set_pos(&mut self, pos: CursorPosition);
}
