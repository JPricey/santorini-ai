#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GenericMove(pub u64);

// Somehow sharing this between multiple levels is slower than creating a new vec each time.
// Pretty amazing. Not sure how.
pub struct ParentMoveContainer {
    pub moves: Box<[GenericMove]>,
}

impl ParentMoveContainer {
    pub fn new(capacity: usize) -> Self {
        let mut vec = Vec::with_capacity(capacity);
        unsafe {
            vec.set_len(capacity);
        }

        Self {
            moves: vec.into_boxed_slice(),
        }
    }

    pub fn get_child(&mut self) -> ChildMoveContainer {
        ChildMoveContainer {
            parent_move_container: self,
            head: 0,
            tail: 0,
        }
    }
}

impl Default for ParentMoveContainer {
    fn default() -> Self {
        Self::new(65536)
    }
}

pub struct ChildMoveContainer<'a> {
    parent_move_container: &'a mut ParentMoveContainer,
    head: usize,
    tail: usize,
}

impl<'a> ChildMoveContainer<'a> {
    pub fn get_child(&mut self) -> ChildMoveContainer {
        ChildMoveContainer {
            parent_move_container: self.parent_move_container,
            head: self.tail,
            tail: self.tail,
        }
    }

    pub fn push(&mut self, action: GenericMove) {
        self.parent_move_container.moves[self.tail] = action;
        self.tail += 1;
    }

    pub fn consume(&mut self) -> Option<GenericMove> {
        if self.tail <= self.head {
            None
        } else {
            self.tail -= 1;
            Some(self.parent_move_container.moves[self.tail])
        }
    }
}
