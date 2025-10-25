use itertools::Itertools;

use crate::{
    bitboard::{BitBoard, LOWER_SQUARES_EXCLUSIVE_MASK, PERIMETER_SPACES_MASK},
    board::{BoardState, GodPair},
    gods::{
        FullAction, PartialAction,
        generic::{GenericMove, GodMove, LOWER_POSITION_MASK, MoveData, POSITION_WIDTH},
    },
    placement::common::{WorkerPlacementMove, compute_unique_placements},
    player::Player,
    square::Square,
};

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct PerimeterWorkerPlacement(MoveData);

impl std::fmt::Debug for PerimeterWorkerPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P{} P{}", self.placement_1(), self.placement_2())
    }
}

impl Into<GenericMove> for PerimeterWorkerPlacement {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for PerimeterWorkerPlacement {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl PerimeterWorkerPlacement {
    const fn new(a: Square, b: Square) -> Self {
        let data: MoveData = ((a as MoveData) << 0) | ((b as MoveData) << POSITION_WIDTH);

        Self(data)
    }

    fn placement_1(self) -> Square {
        let pos = self.0 as u8 & LOWER_POSITION_MASK;
        Square::from(pos)
    }

    fn placement_2(self) -> Square {
        let pos = (self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK;
        Square::from(pos)
    }
}

impl GodMove for PerimeterWorkerPlacement {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let actions = vec![
            PartialAction::PlaceWorker(self.placement_1()),
            PartialAction::PlaceWorker(self.placement_2()),
        ];

        let result: Vec<FullAction> = actions.into_iter().permutations(2).collect();

        result
    }

    fn make_move(self, board: &mut BoardState, player: Player) {
        self.make_move_no_swap_sides(board, player);
        board.flip_current_player();
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        BitBoard::EMPTY
    }

    fn get_history_idx(self, _board: &BoardState) -> usize {
        self.0 as usize
    }
}

impl WorkerPlacementMove for PerimeterWorkerPlacement {
    fn make_move_no_swap_sides(&self, board: &mut BoardState, player: Player) {
        board.worker_xor(
            player,
            self.placement_1().to_board() | self.placement_2().to_board(),
        );
    }

    fn get_all_placements(_gods: GodPair, board: &BoardState, player: Player) -> Vec<GenericMove> {
        let mut valid_squares = PERIMETER_SPACES_MASK;
        valid_squares &= !board.workers[!player as usize];

        let n = valid_squares.count_ones() as usize;
        let capacity = n * (n - 1) / 2;
        let mut res = Vec::with_capacity(capacity);

        for a in valid_squares {
            let b_valids = valid_squares & LOWER_SQUARES_EXCLUSIVE_MASK[a as usize];

            for b in b_valids {
                let action = PerimeterWorkerPlacement::new(a, b);
                res.push(action.into());
            }
        }

        debug_assert!(res.len() == capacity);

        res
    }

    fn get_unique_placements(
        gods: GodPair,
        board: &BoardState,
        player: Player,
    ) -> Vec<GenericMove> {
        compute_unique_placements::<Self>(gods, board, player)
    }
}
