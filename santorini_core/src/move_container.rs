/*
while iter1 {
    iter2 = new()
    while iter2 {
        ...
    }
}
*/

/*
res = iter2.next()
if res is None

let i1 = iter1.next()
if i1 is None:
return None
iter2 = i1
*/

/*
struct MortalWorkerMove {

}

struct MortalMoveIterator<'a> {
    pub workers_to_move_iter: BitBoard,
    pub current_moves_iter: BitBoard,
    pub board: &'a BoardState,
}

impl<'a> MortalMoveIterator<'a> {
    pub fn new(board: &'a BoardState, workers_to_move: BitBoard) -> Self {
        Self {
            workers_to_move_iter: workers_to_move.into_iter(),
            board,
        }
    }
}

impl<'a> Iterator for MortalMoveIterator<'a> {
    type Item = MortalWorkerMove;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
*/

use crate::gods::generic::GenericMove;


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
    pub fn get_child(&mut self) -> ChildMoveContainer<'_> {
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
