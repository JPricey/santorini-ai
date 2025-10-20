use itertools::Itertools;

use crate::{
    bitboard::{BitBoard, LOWER_SQUARES_EXCLUSIVE_MASK},
    board::{BoardState, GodPair},
    gods::{
        generic::{GenericMove, GodMove, MoveData, LOWER_POSITION_MASK, POSITION_WIDTH}, FullAction, PartialAction, StaticGod
    },
    placement::common::{compute_unique_placements, WorkerPlacementMove},
    player::Player,
    square::Square,
};

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct ThreeWorkerPlacement(MoveData);

impl std::fmt::Debug for ThreeWorkerPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "P{} P{} P{}",
            self.placement_1(),
            self.placement_2(),
            self.placement_3()
        )
    }
}

impl Into<GenericMove> for ThreeWorkerPlacement {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for ThreeWorkerPlacement {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl ThreeWorkerPlacement {
    const fn new(a: Square, b: Square, c: Square) -> Self {
        let data: MoveData = ((a as MoveData) << 0)
            | ((b as MoveData) << POSITION_WIDTH)
            | ((c as MoveData) << (2 * POSITION_WIDTH));

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

    fn placement_3(self) -> Square {
        let pos = (self.0 >> 2 * POSITION_WIDTH) as u8 & LOWER_POSITION_MASK;
        Square::from(pos)
    }
}

impl GodMove for ThreeWorkerPlacement {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let actions = vec![
            PartialAction::PlaceWorker(self.placement_1()),
            PartialAction::PlaceWorker(self.placement_2()),
            PartialAction::PlaceWorker(self.placement_3()),
        ];

        let result: Vec<FullAction> = actions.into_iter().permutations(3).collect();

        result
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
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

impl WorkerPlacementMove for ThreeWorkerPlacement {
    fn make_move_no_swap_sides(&self, board: &mut BoardState, player: Player) {
        board.worker_xor(
            player,
            self.placement_1().to_board()
                | self.placement_2().to_board()
                | self.placement_3().to_board(),
        );
    }

    fn get_all_placements(_gods: GodPair, board: &BoardState, player: Player) -> Vec<GenericMove> {
        let mut valid_squares = BitBoard::MAIN_SECTION_MASK;
        valid_squares &= !board.workers[!player as usize];

        let n = valid_squares.count_ones() as usize;
        let capacity = n * (n - 1) * (n - 2) / 6;
        let mut res = Vec::with_capacity(capacity);

        for a in valid_squares {
            let b_valids = valid_squares & LOWER_SQUARES_EXCLUSIVE_MASK[a as usize];

            for b in b_valids {
                let c_valids = valid_squares & LOWER_SQUARES_EXCLUSIVE_MASK[b as usize];

                for c in c_valids {
                    let action = ThreeWorkerPlacement::new(a, b, c);
                    res.push(action.into());
                }
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
