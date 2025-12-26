use itertools::Itertools;

use crate::{
    bitboard::BitBoard,
    board::{BoardState, GodPair},
    gods::{
        FullAction, PartialAction, StaticGod,
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
    fn move_to_actions(self, _board: &BoardState, _player: Player, _other_god: StaticGod) -> Vec<FullAction> {
        let actions = vec![
            PartialAction::PlaceWorker(self.placement_1()),
            PartialAction::PlaceWorker(self.placement_2()),
        ];

        let result: Vec<FullAction> = actions.into_iter().permutations(2).collect();

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

const BOT: BitBoard = BitBoard(0b11111_00000_00000_00000_00000);
const RIGHT: BitBoard = BitBoard(0b10000_10000_10000_10000_10000);

const BOT_RIGHT: BitBoard = RIGHT.bit_or(BOT);

fn _add_placements(a: Square, b_board: BitBoard, res: &mut Vec<GenericMove>) {
    for b in b_board {
        let action = OppositeWorkerPlacement::new(a, b);
        res.push(action.into());
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
        let open_squares = !board.workers[!player as usize];

        let mut res = Vec::with_capacity(49);

        _add_placements(Square::A5, open_squares & BOT_RIGHT, &mut res);

        _add_placements(Square::B5, open_squares & BOT, &mut res);
        _add_placements(Square::C5, open_squares & BOT, &mut res);
        _add_placements(Square::D5, open_squares & BOT, &mut res);
        _add_placements(Square::E5, open_squares & BOT, &mut res);

        _add_placements(Square::A4, open_squares & RIGHT, &mut res);
        _add_placements(Square::A3, open_squares & RIGHT, &mut res);
        _add_placements(Square::A2, open_squares & RIGHT, &mut res);
        _add_placements(Square::A1, open_squares & RIGHT, &mut res);

        debug_assert!(res.len() <= 49);

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
