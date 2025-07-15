use crate::{
    board::BoardState,
    gods::{
        StaticGod,
        generic::{GenericMove, NON_IMPROVER_SENTINEL_SCORE, ScoredMove},
    },
    player::Player,
};

pub const MAX_MOVE_COUNT: usize = 336;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MovePickerStage {
    YieldTT,
    GenerateAllMoves,
    ScoreImprovers,
    YieldImprovers,
    YieldKiller,
    ScoreNonImprovers,
    YieldNonImprovers,
    Done,
}

pub struct MovePicker {
    player: Player,
    active_god: StaticGod,
    move_list: Vec<ScoredMove>,
    tt_move: Option<GenericMove>,
    killer_move: Option<GenericMove>,
    pub stage: MovePickerStage,
    index: usize,
}

impl MovePicker {
    pub fn new(
        player: Player,
        active_god: StaticGod,
        tt_move: Option<GenericMove>,
        killer_move: Option<GenericMove>,
    ) -> Self {
        Self {
            player,
            active_god,
            move_list: Default::default(),
            tt_move: tt_move.filter(|e| *e != GenericMove::NULL_MOVE),
            killer_move,
            stage: MovePickerStage::YieldTT,
            index: 0,
        }
    }

    fn _generate_moves(&mut self, board: &BoardState) {
        self.move_list = self.active_god.get_moves_for_search(board, self.player);
    }

    pub fn has_any_moves(&mut self, board: &BoardState) -> bool {
        if self.stage == MovePickerStage::YieldTT {
            if self.tt_move.is_some() {
                return true;
            }

            self.stage = MovePickerStage::GenerateAllMoves;
        }

        if self.stage == MovePickerStage::GenerateAllMoves {
            self.stage = MovePickerStage::ScoreImprovers;
            self._generate_moves(board);
        }

        self.move_list.len() > 0
    }

    pub fn get_winning_move(&mut self, board: &BoardState) -> Option<GenericMove> {
        if self.stage == MovePickerStage::YieldTT {
            // We don't save tt entries for winning moves, so no need to even check it
            if self.tt_move.is_some() {
                return None;
            } else {
                self.stage = MovePickerStage::GenerateAllMoves;
            }
        }

        if self.stage == MovePickerStage::GenerateAllMoves {
            self.stage = MovePickerStage::ScoreImprovers;
            self._generate_moves(board);
        }

        // get_moves_for_search stops running once it sees a win, so if there is a win it'll be last
        if let Some(last_move) = self.move_list.last() {
            if last_move.get_is_winning() {
                return Some(last_move.action.clone());
            }
        }

        return None;
    }

    pub fn next(&mut self, board: &BoardState) -> Option<GenericMove> {
        if self.stage == MovePickerStage::YieldTT {
            self.stage = MovePickerStage::GenerateAllMoves;
            // TODO: protect against a hash collision by confirming move validity??
            if self.tt_move.is_some() {
                return self.tt_move;
            }
        }

        if self.stage == MovePickerStage::GenerateAllMoves {
            self.stage = MovePickerStage::ScoreImprovers;
            self._generate_moves(board);
        }

        if self.stage == MovePickerStage::ScoreImprovers {
            self.stage = MovePickerStage::YieldImprovers;
            self.active_god
                .score_improvers(board, &mut self.move_list[self.index..]);
        }

        if self.stage == MovePickerStage::YieldImprovers {
            if self.index >= self.move_list.len() {
                self.stage = MovePickerStage::Done;
                return None;
            }

            let mut best_index = self.index;
            let mut best_score = self.move_list[best_index].score;
            for i in best_index + 1..self.move_list.len() {
                if self.move_list[i].score > best_score {
                    best_index = i;
                    best_score = self.move_list[i].score;
                }
            }

            if best_score <= NON_IMPROVER_SENTINEL_SCORE {
                self.stage = MovePickerStage::YieldKiller;
            } else {
                if best_index != self.index {
                    self.move_list.swap(self.index, best_index);
                }

                let result_move = Some(self.move_list[self.index].action);
                self.index += 1;

                if result_move == self.tt_move {
                    return self.next(board);
                } else {
                    return result_move;
                }
            }
        }

        if self.stage == MovePickerStage::YieldKiller {
            self.stage = MovePickerStage::ScoreNonImprovers;
            if self.killer_move != self.tt_move
                && let Some(killer_move) = self.killer_move
            {
                if let Some(killer_index) =
                    self.move_list.iter().position(|m| m.action == killer_move)
                {
                    if killer_index > self.index {
                        self.move_list.swap(self.index, killer_index);
                    }
                    self.index += 1;
                    return Some(killer_move);
                }
            }
        }

        if self.stage == MovePickerStage::ScoreNonImprovers {
            self.stage = MovePickerStage::YieldNonImprovers;
            self.active_god
                .score_remaining(board, &mut self.move_list[self.index..]);
        }

        if self.stage == MovePickerStage::YieldNonImprovers {
            if self.index >= self.move_list.len() {
                self.stage = MovePickerStage::Done;
                return None;
            }
            let mut best_index = self.index;
            let mut best_score = self.move_list[best_index].score;
            for i in best_index + 1..self.move_list.len() {
                if self.move_list[i].score > best_score {
                    best_index = i;
                    best_score = self.move_list[i].score;
                }
            }

            if best_index != self.index {
                self.move_list.swap(self.index, best_index);
            }

            let result_move = Some(self.move_list[self.index].action);
            self.index += 1;

            if result_move == self.tt_move {
                return self.next(board);
            } else {
                return result_move;
            }
        }

        panic!("Unreachable picker state! {:?}", self.stage);
    }
}
