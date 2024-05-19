use std::cmp::min;
use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::cursor::{Cursor, CursorPosition, CursorWithPosition};

#[derive(Debug)]
pub(crate) struct SharedVecCursor<T> {
    vec: Arc<Vec<T>>,
    // As an optimization to avoid an enum and make navigation code less
    // complex, we use 0 to represent the "start" state and vec.len()+1 to
    // represent the "end" state. Values in between represent 1-based indices
    // into the slice. This means that slices cannot be larger than
    // usize::MAX-2.
    pos: usize,
}

impl<T> Clone for SharedVecCursor<T> {
    fn clone(&self) -> Self {
        SharedVecCursor {
            vec: self.vec.clone(),
            pos: self.pos,
        }
    }
}

impl<T> PartialEq for SharedVecCursor<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.vec, &other.vec) && self.pos == other.pos
    }
}

impl<T> SharedVecCursor<T> {
    pub fn get_shared_vec(&self) -> Arc<Vec<T>> {
        self.vec.clone()
    }
}

impl<T> SharedVecCursor<T> {
    pub fn at_start(vec: Arc<Vec<T>>) -> Self {
        Self::check_size(&vec);
        SharedVecCursor { vec, pos: 0 }
    }

    pub fn at_end(vec: Arc<Vec<T>>) -> Self {
        Self::check_size(&vec);
        SharedVecCursor {
            pos: vec.len() + 1,
            vec,
        }
    }

    pub fn with_pos(vec: Arc<Vec<T>>, pos: usize) -> Self {
        Self::check_size(&vec);
        assert!(pos < vec.len(), "Position out of bounds");
        SharedVecCursor { vec, pos: pos + 1 }
    }

    fn check_size(slice: &[T]) {
        assert!(slice.len() < usize::MAX, "Vec too large");
    }
}

impl<T> Cursor for SharedVecCursor<T> {
    type Item = T;

    fn next(&mut self) -> Option<&Self::Item> {
        let end = self.vec.len() + 1;
        if self.pos < end {
            self.pos += 1;
            if self.pos < end {
                Some(&self.vec[self.pos - 1])
            } else {
                None
            }
        } else {
            None
        }
    }

    fn advance_by(&mut self, n: usize) -> Result<(), std::num::NonZeroUsize> {
        let remaining = self.vec.len() - self.pos;
        let advance = min(n, remaining);
        self.pos += advance;
        let n = n - advance;
        if n > 0 {
            Err(NonZeroUsize::new(n).unwrap())
        } else {
            Ok(())
        }
    }

    fn prev(&mut self) -> Option<&Self::Item> {
        if self.pos > 0 {
            self.pos -= 1;
            if self.pos > 0 {
                Some(&self.vec[self.pos - 1])
            } else {
                None
            }
        } else {
            None
        }
    }

    fn current(&self) -> Option<&Self::Item> {
        if self.pos > 0 && self.pos <= self.vec.len() {
            Some(&self.vec[self.pos - 1])
        } else {
            None
        }
    }

    fn peek_next(&self) -> Option<&Self::Item> {
        if self.pos < self.vec.len() {
            Some(&self.vec[self.pos])
        } else {
            None
        }
    }

    fn peek_prev(&self) -> Option<&Self::Item> {
        if self.pos > 1 {
            Some(&self.vec[self.pos - 1 - 1])
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
        self.pos == self.vec.len() + 1
    }
}

impl<T> CursorWithPosition for SharedVecCursor<T> {
    fn pos(&self) -> CursorPosition {
        if self.pos == 0 {
            CursorPosition::Start
        } else if self.pos == self.vec.len() + 1 {
            CursorPosition::End
        } else {
            CursorPosition::Pos(self.pos - 1)
        }
    }

    fn set_pos(&mut self, pos: CursorPosition) {
        match pos {
            CursorPosition::Start => self.pos = 0,
            CursorPosition::End => self.pos = self.vec.len() + 1,
            CursorPosition::Pos(pos) => self.pos = pos + 1,
        }
    }
}

// TODO tests; see slice_cursor.rs.
