use std::cmp::min;
use std::num::NonZeroUsize;

use crate::cursor::{Cursor, CursorPosition, CursorWithPosition};

pub(crate) struct SliceCursor<'a, T> {
    slice: &'a [T],
    // As an optimization to avoid an enum and make navigation code less
    // complex, we use 0 to represent the "start" state and slice.len()+1 to
    // represent the "end" state. Values in between represent 1-based indices
    // into the slice. This means that slices cannot be larger than
    // usize::MAX-2.
    pos: usize,
}

impl<'a, T> SliceCursor<'a, T> {
    pub fn at_start(slice: &'a [T]) -> Self {
        Self::check_size(slice);
        SliceCursor { slice, pos: 0 }
    }

    pub fn at_end(slice: &'a [T]) -> Self {
        Self::check_size(slice);
        SliceCursor {
            slice,
            pos: slice.len() + 1,
        }
    }

    pub fn with_pos(slice: &'a [T], pos: usize) -> Self {
        Self::check_size(slice);
        assert!(pos < slice.len(), "Position out of bounds");
        SliceCursor {
            slice,
            pos: pos + 1,
        }
    }

    fn check_size(slice: &[T]) {
        assert!(slice.len() < usize::MAX, "Slice too large");
    }
}

impl<'a, T> Iterator for SliceCursor<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        let end = self.slice.len() + 1;
        if self.pos < end {
            self.pos += 1;
            if self.pos < end {
                Some(&self.slice[self.pos - 1])
            } else {
                None
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.pos == 0 {
            (self.slice.len(), Some(self.slice.len()))
        } else if self.pos == self.slice.len() + 1 {
            (0, Some(0))
        } else {
            let remaining = self.slice.len() - self.pos;
            (remaining, Some(remaining))
        }
    }
}

impl<'a, T> Cursor for SliceCursor<'a, T> {
    fn advance_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
        let remaining = self.slice.len() - self.pos;
        let advance = min(n, remaining);
        self.pos += advance;
        let n = n - advance;
        if n > 0 {
            Err(NonZeroUsize::new(n).unwrap())
        } else {
            Ok(())
        }
    }

    fn prev(&mut self) -> Option<Self::Item> {
        if self.pos > 0 {
            self.pos -= 1;
            if self.pos > 0 {
                Some(&self.slice[self.pos - 1])
            } else {
                None
            }
        } else {
            None
        }
    }

    fn current(&self) -> Option<Self::Item> {
        if self.pos > 0 && self.pos <= self.slice.len() {
            Some(&self.slice[self.pos - 1])
        } else {
            None
        }
    }

    fn peek_next(&self) -> Option<Self::Item> {
        if self.pos < self.slice.len() {
            Some(&self.slice[self.pos])
        } else {
            None
        }
    }

    fn peek_prev(&self) -> Option<Self::Item> {
        if self.pos > 1 {
            Some(&self.slice[self.pos - 1 - 1])
        } else {
            None
        }
    }

    fn revert_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
        let remaining = self.pos - 1;
        let revert = min(n, remaining);
        self.pos -= revert;
        let n = n - revert;
        if n > 0 {
            Err(NonZeroUsize::new(n).unwrap())
        } else {
            Ok(())
        }
    }

    fn at_start(&self) -> bool {
        self.pos == 0
    }

    fn at_end(&self) -> bool {
        self.pos == self.slice.len() + 1
    }
}

impl<'a, T> CursorWithPosition for SliceCursor<'a, T> {
    fn pos(&self) -> CursorPosition {
        if self.pos == 0 {
            CursorPosition::Start
        } else if self.pos == self.slice.len() + 1 {
            CursorPosition::End
        } else {
            CursorPosition::Pos(self.pos - 1)
        }
    }

    fn set_pos(&mut self, pos: CursorPosition) {
        match pos {
            CursorPosition::Start => self.pos = 0,
            CursorPosition::End => self.pos = self.slice.len() + 1,
            CursorPosition::Pos(pos) => self.pos = pos + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_navigation() {
        let slice = &[1, 2, 3];

        // Forward direction.
        let mut cursor = SliceCursor::at_start(slice);
        assert_eq!(cursor.peek_next(), Some(&1));
        assert_eq!(cursor.next(), Some(&1));
        assert_eq!(cursor.current(), Some(&1));
        assert_eq!(cursor.peek_next(), Some(&2));
        assert_eq!(cursor.next(), Some(&2));
        assert_eq!(cursor.next(), Some(&3));
        assert_eq!(cursor.peek_next(), None);
        assert_eq!(cursor.current(), Some(&3));
        assert_eq!(cursor.next(), None);
        assert_eq!(cursor.current(), None);
        assert_eq!(cursor.peek_prev(), Some(&3));

        // Backward direction.
        let mut cursor = SliceCursor::at_end(slice);
        assert_eq!(cursor.peek_prev(), Some(&3));
        assert_eq!(cursor.prev(), Some(&3));
        assert_eq!(cursor.current(), Some(&3));
        assert_eq!(cursor.prev(), Some(&2));
        assert_eq!(cursor.prev(), Some(&1));
        assert_eq!(cursor.peek_prev(), None);
        assert_eq!(cursor.current(), Some(&1));
        assert_eq!(cursor.prev(), None);
        assert_eq!(cursor.current(), None);
        assert_eq!(cursor.peek_next(), Some(&1));

        // Forward then backward on the same cursor.
        let mut cursor = SliceCursor::at_start(slice);
        assert_eq!(cursor.next(), Some(&1));
        assert_eq!(cursor.next(), Some(&2));
        assert_eq!(cursor.next(), Some(&3));
        assert_eq!(cursor.next(), None);
        assert_eq!(cursor.prev(), Some(&3));
        assert_eq!(cursor.prev(), Some(&2));
        assert_eq!(cursor.prev(), Some(&1));
        assert_eq!(cursor.prev(), None);

        assert_eq!(cursor.next(), Some(&1));
        assert_eq!(cursor.prev(), None);
        assert_eq!(cursor.next(), Some(&1));
        assert_eq!(cursor.next(), Some(&2));
        assert_eq!(cursor.prev(), Some(&1));

        // Some back and forth edge cases.
        let mut cursor = SliceCursor::at_end(slice);
        assert_eq!(cursor.prev(), Some(&3));
        assert_eq!(cursor.next(), None);
        assert_eq!(cursor.prev(), Some(&3));
        assert_eq!(cursor.prev(), Some(&2));
        assert_eq!(cursor.next(), Some(&3));

        // At specific positions.
        let cursor = SliceCursor::with_pos(slice, 0);
        assert_eq!(cursor.current(), Some(&1));
        let cursor = SliceCursor::with_pos(slice, 1);
        assert_eq!(cursor.current(), Some(&2));
        let cursor = SliceCursor::with_pos(slice, 2);
        assert_eq!(cursor.current(), Some(&3));
    }

    #[test]
    fn empty_slice() {
        let slice: &[i32] = &[];
        let mut cursor = SliceCursor::at_start(slice);
        assert_eq!(cursor.peek_next(), None);
        assert_eq!(cursor.next(), None);
        assert_eq!(cursor.current(), None);
        assert_eq!(cursor.next(), None);
        assert_eq!(cursor.prev(), None);
        assert_eq!(cursor.current(), None);
        assert_eq!(cursor.prev(), None);
    }

    #[test]
    fn advance_by_test() {
        let slice = &[1, 2, 3];
        let mut cursor = SliceCursor::at_start(slice);

        // "advance_by(n) will return Ok(()) if the iterator successfully advances by n elements,
        // or a Err(NonZeroUsize) with value k if None is encountered, where k is remaining number
        // of steps that could not be advanced because the iterator ran out. If self is empty and
        // n is non-zero, then this returns Err(n). Otherwise, k is always less than n."
        //
        // Start by doing this using next(). Then do it using advance_by() and check that the
        // results are the same.
        let mut expected = 0;
        for _ in 0..7 {
            if cursor.next() == None {
                expected += 1;
            }
        }
        let mut cursor = SliceCursor::at_start(slice);
        assert_eq!(
            Cursor::advance_by(&mut cursor, 7),
            Err(NonZeroUsize::new(expected).unwrap())
        );

        let mut cursor = SliceCursor::at_start(&[] as &[i32; 0]);
        assert_eq!(
            Cursor::advance_by(&mut cursor, 7),
            Err(NonZeroUsize::new(7).unwrap())
        );
    }

    #[test]
    fn revert_by_test() {
        let slice = &[1, 2, 3];
        let mut cursor = SliceCursor::at_end(slice);

        // revert_by does not have an std counterpart, but we expect the behavior to be the same
        // as advance_by, but in the opposite direction.
        let mut expected = 0;
        for _ in 0..7 {
            if cursor.prev() == None {
                expected += 1;
            }
        }
        let mut cursor = SliceCursor::at_end(slice);
        assert_eq!(
            cursor.revert_by(7),
            Err(NonZeroUsize::new(expected).unwrap())
        );

        let mut cursor = SliceCursor::at_end(&[] as &[i32; 0]);
        assert_eq!(cursor.revert_by(7), Err(NonZeroUsize::new(7).unwrap()));
    }
}
