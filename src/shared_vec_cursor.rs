use std::cmp::min;
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::cursor::{Cursor, CursorPosition, CursorWithPosition};

pub(crate) struct SharedVecCursor<'a, T> {
    vec: Arc<Vec<T>>,
    // As an optimization to avoid an enum and make navigation code less
    // complex, we use 0 to represent the "start" state and vec.len()+1 to
    // represent the "end" state. Values in between represent 1-based indices
    // into the slice. This means that slices cannot be larger than
    // usize::MAX-2.
    pos: usize,
    phantom: PhantomData<&'a T>,
}

impl<'a, T> SharedVecCursor<'a, T> {
    pub fn at_start(vec: &Arc<Vec<T>>) -> Self {
        Self::check_size(vec);
        SharedVecCursor {
            vec: vec.clone(),
            pos: 0,
            phantom: PhantomData,
        }
    }

    pub fn at_end(vec: &Arc<Vec<T>>) -> Self {
        Self::check_size(vec);
        SharedVecCursor {
            vec: vec.clone(),
            pos: vec.len() + 1,
            phantom: PhantomData,
        }
    }

    pub fn with_pos(vec: &Arc<Vec<T>>, pos: usize) -> Self {
        Self::check_size(vec);
        assert!(pos < vec.len(), "Position out of bounds");
        SharedVecCursor {
            vec: vec.clone(),
            pos: pos + 1,
            phantom: PhantomData,
        }
    }

    fn check_size(vec: &Arc<Vec<T>>) {
        assert!(vec.len() < usize::MAX, "Vec too large");
    }

    fn ref_pos(&self, index: usize) -> &'a T {
        let p: *const T = &self.vec[index];
        // SAFETY: I haven't figured out how to express that the Arc
        // instance is borrowed for the lifetime of the cursor, so I'm
        // using an unsafe block here. The Arc instance lives for as
        // long as the cursor does - we never drop/replace the Arc
        // instance while the cursor is still alive. Arc does not allow
        // interior mutability, so the vector it points to is immutable.
        unsafe { &*p }
    }
}

impl<'a, T> Iterator for SharedVecCursor<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        let end = self.vec.len() + 1;
        if self.pos < end {
            self.pos += 1;
            if self.pos < end {
                Some(self.ref_pos(self.pos - 1))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.pos == 0 {
            (self.vec.len(), Some(self.vec.len()))
        } else if self.pos == self.vec.len() + 1 {
            (0, Some(0))
        } else {
            let remaining = self.vec.len() - self.pos;
            (remaining, Some(remaining))
        }
    }
}

impl<'a, T> Cursor for SharedVecCursor<'a, T> {
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

    fn prev(&mut self) -> Option<Self::Item> {
        if self.pos > 0 {
            self.pos -= 1;
            if self.pos > 0 {
                Some(self.ref_pos(self.pos - 1))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn current(&self) -> Option<Self::Item> {
        if self.pos > 0 && self.pos <= self.vec.len() {
            Some(self.ref_pos(self.pos - 1))
        } else {
            None
        }
    }

    fn peek_next(&self) -> Option<Self::Item> {
        if self.pos < self.vec.len() {
            Some(self.ref_pos(self.pos))
        } else {
            None
        }
    }

    fn peek_prev(&self) -> Option<Self::Item> {
        if self.pos > 1 {
            Some(self.ref_pos(self.pos - 1 - 1))
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

impl<'a, T> CursorWithPosition for SharedVecCursor<'a, T> {
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
