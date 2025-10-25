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
pub(crate) struct OppositeWorkerPlacement(MoveData);

impl std::fmt::Debug for OppositeWorkerPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P{} P{}", self.placement_1(), self.placement_2())
    }
}

impl Into<GenericMove> for OppositeWorkerPlacement {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for OppositeWorkerPlacement {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl OppositeWorkerPlacement {
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

impl GodMove for OppositeWorkerPlacement {
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

impl WorkerPlacementMove for OppositeWorkerPlacement {
    fn make_move_no_swap_sides(&self, board: &mut BoardState, player: Player) {
        board.worker_xor(
            player,
            self.placement_1().to_board() | self.placement_2().to_board(),
        );
    }

    fn get_all_placements(_gods: GodPair, board: &BoardState, player: Player) -> Vec<GenericMove> {
        let mut valid_starters = PERIMETER_SPACES_MASK & !LOWER_SQUARES_EXCLUSIVE_MASK[12 as usize];
        let valid_anywhere = !board.workers[!player as usize];
        valid_starters &= valid_anywhere;

        let mut res = Vec::with_capacity(valid_starters.count_ones() as usize);

        for a in valid_starters {
            let b = Square::from(24 - (a as u8));
            if valid_anywhere.contains_square(b) {
                let action = OppositeWorkerPlacement::new(a, b);
                res.push(action.into());
            }
        }

        debug_assert!(res.len() <= valid_starters.count_ones() as usize);

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
