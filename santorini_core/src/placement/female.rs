use crate::{
    bitboard::{BitBoard, LOWER_SQUARES_EXCLUSIVE_MASK},
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
pub(crate) struct FemaleWorkerPlacement(MoveData);

impl std::fmt::Debug for FemaleWorkerPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "F{} M{}", self.female_worker(), self.male_worker())
    }
}

impl Into<GenericMove> for FemaleWorkerPlacement {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for FemaleWorkerPlacement {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl FemaleWorkerPlacement {
    const fn new(f: Square, m: Square) -> Self {
        let data: MoveData = ((f as MoveData) << 0) | ((m as MoveData) << POSITION_WIDTH);

        Self(data)
    }

    fn female_worker(self) -> Square {
        let pos = self.0 as u8 & LOWER_POSITION_MASK;
        Square::from(pos)
    }

    fn male_worker(self) -> Square {
        let pos = (self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK;
        Square::from(pos)
    }
}

impl GodMove for FemaleWorkerPlacement {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        vec![
            vec![
                PartialAction::PlaceWorker(self.female_worker()),
                PartialAction::PlaceWorker(self.male_worker()),
                PartialAction::SetFemaleWorker(self.female_worker()),
            ],
            vec![
                PartialAction::PlaceWorker(self.male_worker()),
                PartialAction::PlaceWorker(self.female_worker()),
                PartialAction::SetFemaleWorker(self.female_worker()),
            ],
        ]
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

impl WorkerPlacementMove for FemaleWorkerPlacement {
    fn make_move_no_swap_sides(&self, board: &mut BoardState, player: Player) {
        board.worker_xor(
            player,
            self.female_worker().to_board() | self.male_worker().to_board(),
        );
        board.set_god_data(player, self.female_worker().to_board().0);
    }

    fn get_all_placements(_gods: GodPair, board: &BoardState, player: Player) -> Vec<GenericMove> {
        let mut valid_squares = BitBoard::MAIN_SECTION_MASK;
        valid_squares &= !board.workers[!player as usize];

        let n = valid_squares.count_ones() as usize;
        let capacity = n * (n - 1);
        let mut res = Vec::with_capacity(capacity);

        for a in valid_squares {
            let b_valids = valid_squares & LOWER_SQUARES_EXCLUSIVE_MASK[a as usize];

            for b in b_valids {
                res.push(FemaleWorkerPlacement::new(a, b).into());
                res.push(FemaleWorkerPlacement::new(b, a).into());
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
