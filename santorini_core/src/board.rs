use core::panic;

use colored::Colorize;
use strum::IntoEnumIterator;

use crate::{
    bitboard::BitBoard,
    fen::{game_state_to_fen, parse_fen},
    gods::{BoardStateWithAction, GameStateWithAction, GodName, StaticGod, generic::GenericMove},
    hashing::{
        HashType, ZOBRIST_DATA_RANDOMS, ZOBRIST_HEIGHT_RANDOMS, ZOBRIST_PLAYER_TWO,
        ZOBRIST_WORKER_RANDOMS, compute_hash_from_scratch_for_board,
    },
    matchup::{self, BANNED_MATCHUPS, Matchup},
    placement::{PlacementType, get_starting_placement_state},
    player::Player,
    square::Square,
};

use serde::{Deserialize, Serialize};

pub type GodData = u32;
pub type GodPair = [StaticGod; 2];

#[derive(Clone, PartialEq, Eq)]
pub struct FullGameState {
    pub board: BoardState,
    pub gods: GodPair,
}

impl Serialize for FullGameState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let fen = game_state_to_fen(self);
        serializer.serialize_str(&fen)
    }
}

impl<'de> Deserialize<'de> for FullGameState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let fen: String = Deserialize::deserialize(deserializer)?;
        parse_fen(&fen).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for FullGameState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", game_state_to_fen(self))
    }
}

impl std::fmt::Debug for FullGameState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl TryFrom<&str> for FullGameState {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        parse_fen(s)
    }
}

impl TryFrom<&String> for FullGameState {
    type Error = String;

    fn try_from(s: &String) -> Result<Self, Self::Error> {
        parse_fen(s)
    }
}

impl FullGameState {
    pub fn new(board: BoardState, gods: GodPair) -> Self {
        let mut res = FullGameState { gods, board };
        res.recalculate_internals();
        res
    }

    pub fn new_for_matchup(matchup: &Matchup) -> Self {
        FullGameState::new(BoardState::default(), [matchup.god_1(), matchup.god_2()])
    }

    pub fn new_empty_state(p1: GodName, p2: GodName) -> Self {
        FullGameState::new(BoardState::default(), [p1.to_power(), p2.to_power()])
    }

    pub fn new_basic_state(p1: GodName, p2: GodName) -> Self {
        FullGameState::new(
            BoardState::new_basic_state(),
            [p1.to_power(), p2.to_power()],
        )
    }

    pub fn new_basic_state_mortals() -> Self {
        FullGameState::new_basic_state(GodName::Mortal, GodName::Mortal)
    }

    pub fn set_matchup(&mut self, matchup: &Matchup) {
        self.gods = matchup.get_gods();
        self.recalculate_internals();
    }

    pub fn get_matchup(&self) -> Matchup {
        Matchup::new_arr([self.gods[0].god_name, self.gods[1].god_name])
    }

    pub fn flip_current_player(&mut self) {
        self.board.flip_current_player();
    }

    pub fn next_state(
        &self,
        god: StaticGod,
        other_god: StaticGod,
        action: GenericMove,
    ) -> FullGameState {
        let mut result = self.clone();
        god.make_move(&mut result.board, other_god, action);
        result
    }

    pub fn next_state_passing(&self, god: StaticGod) -> FullGameState {
        let mut result = self.clone();
        god.make_passing_move(&mut result.board);
        result
    }

    pub fn get_next_states(&self) -> Vec<FullGameState> {
        let active_god = self.get_active_god();
        let board_states_with_action_list = active_god.get_all_next_states(&self);
        board_states_with_action_list
            .into_iter()
            .map(|e| FullGameState::new(e, self.gods))
            .collect()
    }

    pub fn get_token_squares(&self) -> (BitBoard, BitBoard) {
        fn _frozen_squares(state: &FullGameState, player: Player) -> BitBoard {
            let god = state.gods[player as usize];
            god.get_frozen_mask(&state.board, player)
                | god.get_female_worker_mask(&state.board, player)
        }

        (
            _frozen_squares(self, Player::One),
            _frozen_squares(self, Player::Two),
        )
    }

    pub fn get_current_player_consider_placement_mode(&self) -> Player {
        let placement_mode = get_starting_placement_state(&self.board, self.gods).unwrap();
        if let Some(placement_mode) = placement_mode {
            placement_mode.next_placement
        } else {
            self.board.current_player
        }
    }

    pub fn get_all_next_states_with_actions(&self) -> Vec<(FullGameState, GenericMove)> {
        let placement_mode = get_starting_placement_state(&self.board, self.gods).unwrap();

        if let Some(placement_mode) = placement_mode {
            let active_god = self.gods[placement_mode.next_placement as usize];
            active_god
                .get_all_placement_actions(self.gods, &self.board, placement_mode.next_placement)
                .into_iter()
                .map(|a| {
                    (
                        active_god.make_placement_move_on_clone(
                            a,
                            self,
                            placement_mode.next_placement,
                        ),
                        a.into(),
                    )
                })
                .collect()
        } else {
            let (active_god, other_god) = self.get_active_non_active_gods();
            active_god
                .get_moves_for_search(&self, self.board.current_player)
                .into_iter()
                .map(|a| {
                    let mut state_clone = self.clone();
                    active_god.make_move(&mut state_clone.board, other_god, a.action);
                    (state_clone, a.action)
                })
                .collect()
        }
    }

    pub fn get_next_states_interactive(&self) -> Vec<GameStateWithAction> {
        let placement_mode = get_starting_placement_state(&self.board, self.gods).unwrap();

        if let Some(placement_mode) = placement_mode {
            let active_god = self.gods[placement_mode.next_placement as usize];
            let placement_actions = active_god.get_all_placement_actions(
                self.gods,
                &self.board,
                placement_mode.next_placement,
            );
            let mut res: Vec<GameStateWithAction> = Vec::new();

            for p in placement_actions {
                for series in active_god.placement_move_to_actions(p, &self.board) {
                    let new_state = active_god.make_placement_move_on_clone(
                        p,
                        self,
                        placement_mode.next_placement,
                    );
                    let board_state_w_action = BoardStateWithAction::new(new_state.board, series);
                    res.push(GameStateWithAction::new(
                        board_state_w_action,
                        self.gods[0].god_name,
                        self.gods[1].god_name,
                    ));
                }
            }

            res
        } else {
            let active_god = self.get_active_god();
            let board_states_with_action_list = active_god.get_next_states_interactive(&self);

            board_states_with_action_list
                .into_iter()
                .map(|e| GameStateWithAction::new(e, self.gods[0].god_name, self.gods[1].god_name))
                .collect()
        }
    }

    pub fn get_active_non_active_gods(&self) -> (StaticGod, StaticGod) {
        self.get_player_non_player_gods(self.board.current_player)
    }

    pub fn get_player_non_player_gods(&self, player: Player) -> (StaticGod, StaticGod) {
        (self.gods[player as usize], self.gods[!player as usize])
    }

    pub fn get_god_for_player(&self, player: Player) -> StaticGod {
        self.gods[player as usize]
    }

    pub fn get_active_god(&self) -> StaticGod {
        self.get_god_for_player(self.board.current_player)
    }

    pub fn get_other_god(&self) -> StaticGod {
        self.get_god_for_player(!self.board.current_player)
    }

    pub fn print_to_console(&self) {
        eprintln!("{:?}", self);
        self.board.print_to_console();
    }

    pub fn base_hash(&self) -> HashType {
        self.gods[0].hash1 ^ self.gods[1].hash2
    }

    pub fn recalculate_internals(&mut self) {
        self.board.recalculate_internals(self.base_hash());
    }

    pub fn validation_err(&self) -> Result<(), String> {
        self.board.validation_err(self.base_hash(), self.gods)
    }

    pub fn representation_err(&self) -> Result<(), String> {
        self.board.representation_err(self.base_hash(), self.gods)
    }

    pub fn playable_err(&self) -> Result<(), String> {
        self.board.playable_err(self.gods)
    }

    pub fn validate(&self) {
        self.validation_err().unwrap();
    }

    pub fn get_winner(&self) -> Option<Player> {
        self.board.get_winner()
    }

    pub fn get_all_permutations<const INCLUDE_SELF: bool>(&self) -> Vec<BoardState> {
        self.board
            .get_all_permutations::<INCLUDE_SELF>(self.gods, self.base_hash())
    }
}

pub(crate) const WINNER_MASK_OFFSET: usize = 30;
pub(crate) const P1_WINNER: BitBoard = BitBoard(0b01 << WINNER_MASK_OFFSET);
pub(crate) const P2_WINNER: BitBoard = BitBoard(0b10 << WINNER_MASK_OFFSET);
pub(crate) const WINNER_LOOKUP: [Option<Player>; 3] = [None, Some(Player::One), Some(Player::Two)];

pub(crate) const PLAYER_TO_WINNER_LOOKUP: [BitBoard; 2] = [P1_WINNER, P2_WINNER];

/*
 * Bitmap of each level:
 * 0  1  2  3  4
 * 5  6  7  8  9
 * 10 11 12 13 14
 * 15 16 17 18 19
 * 20 21 22 23 24
 *
 * 25 26 27 28 29
 * 30 31
 *
 * bits 25-29 are best kept clear to not mess with worker moving helpers
 *  eg: move_all_workers_one_include_original_workers
 *
 * on board 0:
 * 30-31 represent winners
 * 00 for no winner
 * 10 for player 1 winning
 * 01 for player 2 winning
 * 11 is invalid state
 *
 * on board 1:
 * 30-31 represent how much a worker can move up (for athena)
 * bit 30 represents the amount that player 1 can move up on their next turn
 * bit 31 represents the amount that player 2 can move up on their next turn
 * move of the time these will be "11", they only change to 0 when playing against athena
 */
#[derive(Clone, Debug, Default, Hash)]
pub struct BoardState {
    pub current_player: Player,
    // height_map[L - 1][s] represents if square s is GTE L
    pub height_map: [BitBoard; 4],
    pub workers: [BitBoard; 2],
    pub god_data: [u32; 2],

    pub hash: HashType,
    pub height_lookup: [u8; 25],
}

impl PartialEq for BoardState {
    fn eq(&self, other: &Self) -> bool {
        self.current_player == other.current_player
            && self.height_map == other.height_map
            && self.workers == other.workers
            && self.god_data == other.god_data
    }
}

impl Eq for BoardState {}

impl BoardState {
    pub fn new_basic_state() -> Self {
        let mut result = Self::default();
        result.workers[1].0 |= 1 << 7;
        result.workers[1].0 |= 1 << 17;
        result.workers[0].0 |= 1 << 11;
        result.workers[0].0 |= 1 << 13;
        result
    }

    pub fn recalculate_internals(&mut self, base_hash: HashType) {
        self.hash = compute_hash_from_scratch_for_board(self, base_hash);
        for square in Square::iter() {
            self.height_lookup[square as usize] =
                self._calculate_height(BitBoard::as_mask(square)) as u8;
        }
    }

    pub fn flip_current_player(&mut self) {
        self.current_player = !self.current_player;
        self.hash ^= ZOBRIST_PLAYER_TWO;
    }

    pub fn get_height(&self, position: Square) -> usize {
        self.height_lookup[position as usize] as usize
    }

    fn _calculate_height(&self, position_mask: BitBoard) -> usize {
        ((self.height_map[0] & position_mask).0 << 0
            | (self.height_map[1] & position_mask).0 << 1
            | (self.height_map[2] & position_mask).0 << 2
            | (self.height_map[3] & position_mask).0 << 3)
            .count_ones() as usize
    }

    pub fn get_winner(&self) -> Option<Player> {
        WINNER_LOOKUP[self.height_map[0].0 as usize >> WINNER_MASK_OFFSET]
    }

    pub fn set_winner(&mut self, player: Player) {
        debug_assert_eq!(self.height_map[0].0 as usize >> WINNER_MASK_OFFSET, 0);
        self.height_map[0] ^= PLAYER_TO_WINNER_LOOKUP[player as usize];
        self.hash ^= ZOBRIST_HEIGHT_RANDOMS[0][WINNER_MASK_OFFSET + player as usize];
    }

    pub fn unset_winner(&mut self, player: Player) {
        let player_bit = WINNER_MASK_OFFSET + player as usize;
        debug_assert_eq!(
            self.height_map[0].0 as usize & 1 << player_bit,
            1 << player_bit
        );

        self.height_map[0] ^= PLAYER_TO_WINNER_LOOKUP[player as usize];
        self.hash ^= ZOBRIST_HEIGHT_RANDOMS[0][player_bit];
    }

    pub fn exactly_level_0(&self) -> BitBoard {
        !self.at_least_level_1()
    }

    pub fn exactly_level_1(&self) -> BitBoard {
        self.height_map[0] & !self.height_map[1]
    }

    pub fn exactly_level_2(&self) -> BitBoard {
        self.height_map[1] & !self.height_map[2]
    }

    pub fn exactly_level_3(&self) -> BitBoard {
        self.height_map[2] & !self.height_map[3]
    }

    pub fn exactly_level_n(&self, level: usize) -> BitBoard {
        if level == 0 {
            self.exactly_level_0()
        } else {
            self.height_map[level - 1] & !self.height_map[level]
        }
    }

    pub fn at_least_level_1(&self) -> BitBoard {
        self.height_map[0]
    }

    pub fn at_least_level_2(&self) -> BitBoard {
        self.height_map[1]
    }

    pub fn at_least_level_3(&self) -> BitBoard {
        self.height_map[2]
    }

    pub fn at_least_level_4(&self) -> BitBoard {
        self.height_map[3]
    }

    pub fn at_least_level_n(&self, level: usize) -> BitBoard {
        if level == 0 {
            BitBoard::MAIN_SECTION_MASK
        } else {
            self.height_map[level - 1]
        }
    }

    pub fn worker_xor(&mut self, player: Player, xor: BitBoard) {
        self.workers[player as usize] ^= xor;
        for pos in xor {
            self.hash ^= ZOBRIST_WORKER_RANDOMS[player as usize][pos as usize];
        }
    }

    pub fn oppo_worker_xor(&mut self, other_god: StaticGod, player: Player, xor: BitBoard) {
        if other_god.placement_type == PlacementType::FemaleWorker {
            if (self.god_data[player as usize] & xor.0) != 0 {
                self.delta_god_data(player, xor.0);
            }
        }

        self.worker_xor(player, xor);
    }

    pub fn oppo_worker_kill(&mut self, other_god: StaticGod, player: Player, xor: BitBoard) {
        if other_god.placement_type == PlacementType::FemaleWorker {
            if (self.god_data[player as usize] & xor.0) != 0 {
                self.set_god_data(player, 0);
            }
        }

        self.worker_xor(player, xor);
    }

    pub fn build_up(&mut self, build_position: Square) {
        let build_mask = BitBoard::as_mask(build_position);
        let current_height = self.get_height(build_position);
        self.height_map[current_height] ^= build_mask;
        self.hash ^= ZOBRIST_HEIGHT_RANDOMS[current_height][build_position as usize];
        self.height_lookup[build_position as usize] += 1;
    }

    pub fn double_build_up(&mut self, build_position: Square) {
        let build_mask = BitBoard::as_mask(build_position);
        let current_height = self.get_height(build_position);
        self.height_map[current_height] ^= build_mask;
        self.height_map[current_height + 1] ^= build_mask;
        self.height_lookup[build_position as usize] += 2;

        self.hash ^= ZOBRIST_HEIGHT_RANDOMS[current_height][build_position as usize];
        self.hash ^= ZOBRIST_HEIGHT_RANDOMS[current_height + 1][build_position as usize];
    }

    pub fn dome_up(&mut self, build_position: Square) {
        let build_mask = BitBoard::as_mask(build_position);
        let current_height = self.get_height(build_position);
        for h in current_height..4 {
            self.height_map[h] ^= build_mask;
            self.hash ^= ZOBRIST_HEIGHT_RANDOMS[h][build_position as usize];
        }
        self.height_lookup[build_position as usize] = 4;
    }

    pub fn unbuild(&mut self, build_position: Square) {
        let build_mask = BitBoard::as_mask(build_position);
        let current_height = self.get_height(build_position) - 1;
        self.height_map[current_height] ^= build_mask;
        self.hash ^= ZOBRIST_HEIGHT_RANDOMS[current_height][build_position as usize];
        self.height_lookup[build_position as usize] -= 1;
    }

    pub fn double_unbuild(&mut self, build_position: Square) {
        let build_mask = BitBoard::as_mask(build_position);
        let current_height = self.get_height(build_position) - 1;
        self.height_map[current_height] ^= build_mask;
        self.hash ^= ZOBRIST_HEIGHT_RANDOMS[current_height][build_position as usize];

        self.height_map[current_height - 1] ^= build_mask;
        self.hash ^= ZOBRIST_HEIGHT_RANDOMS[current_height - 1][build_position as usize];

        self.height_lookup[build_position as usize] -= 2;
    }

    pub fn undome(&mut self, build_position: Square, final_height: usize) {
        let build_mask = BitBoard::as_mask(build_position);
        for h in final_height..4 {
            self.height_map[h] ^= build_mask;
            self.hash ^= ZOBRIST_HEIGHT_RANDOMS[h][build_position as usize];
        }
        self.height_lookup[build_position as usize] = final_height as u8;
    }

    pub fn set_god_data(&mut self, player: Player, data: GodData) {
        let old_data = self.god_data[player as usize];
        self.god_data[player as usize] = data;

        let mut delta = old_data ^ data;
        while delta > 0 {
            let lsb = delta.trailing_zeros();
            delta &= delta - 1;
            self.hash ^= ZOBRIST_DATA_RANDOMS[player as usize][lsb as usize];
        }
    }

    pub fn delta_god_data(&mut self, player: Player, mut delta: GodData) {
        self.god_data[player as usize] ^= delta;

        while delta > 0 {
            let lsb = delta.trailing_zeros();
            delta &= delta - 1;
            self.hash ^= ZOBRIST_DATA_RANDOMS[player as usize][lsb as usize];
        }
    }

    pub fn print_for_debugging(&self) {
        for h in 0..4 {
            eprintln!("{h}: {}", self.height_map[h]);
            eprintln!("{:032b}", self.height_map[h].0);
        }
    }

    fn _validate_player(&self, player: Player, _god: StaticGod) -> Result<(), String> {
        let player_idx = player as usize;
        let player_workers = self.workers[player_idx];

        let dome_worker_collide = player_workers & self.height_map[3];
        if dome_worker_collide.is_not_empty() {
            return Err(format!("Player {:?} has workers on domes", player));
        }

        Ok(())
    }

    fn validation_err(&self, base_hash: HashType, gods: GodPair) -> Result<(), String> {
        self.representation_err(base_hash, gods)?;
        self.playable_err(gods)?;
        Ok(())
    }

    fn playable_err(&self, gods: GodPair) -> Result<(), String> {
        // Only perform placement validations when there is no winner
        // To handle cases where you can win by kills
        if self.get_winner().is_none() {
            if let Some(placement_mode) = get_starting_placement_state(self, gods)? {
                if placement_mode.is_swapped {
                    if self.current_player == placement_mode.next_placement {
                        return Err(format!(
                            "Should be player {:?}'s turn to place",
                            !placement_mode.next_placement
                        ));
                    }
                } else {
                    if self.current_player != placement_mode.next_placement {
                        return Err(format!(
                            "Should be player {:?}'s turn to place",
                            placement_mode.next_placement
                        ));
                    }
                }
            }
        }

        let matchup = Matchup::new(gods[0].god_name, gods[1].god_name);
        if let Some(reason) = BANNED_MATCHUPS.get(&matchup) {
            let err_str = match reason {
                matchup::BannedReason::Game => "This matchup is banned",
                matchup::BannedReason::Engine => "This matchup is not yet implemented",
            };
            return Err(err_str.to_owned());
        }

        self._validate_playable_player(Player::One, gods)?;
        self._validate_playable_player(Player::Two, gods)?;

        Ok(())
    }

    fn _validate_playable_player(&self, player: Player, gods: GodPair) -> Result<(), String> {
        let player_idx = player as usize;
        let (own_god, other_god) = (gods[player_idx], gods[1 - player_idx]);
        let own_workers = self.workers[player_idx];
        let worker_count = own_workers.count_ones();

        let oppo_workers = self.workers[1 - player_idx];
        let oppo_count = oppo_workers.count_ones();

        if own_god.god_name == GodName::Selene {
            let f_worker = self.god_data[player as usize];
            if (f_worker & !own_workers.0) > 0 {
                return Err(format!(
                    "Player {:?} as Selene has misaligned female worker",
                    player,
                ));
            }
        }

        if [GodName::Selene, GodName::Europa, GodName::Hippolyta].contains(&own_god.god_name) {
            if self.god_data[player as usize].count_ones() > 1 {
                return Err(format!(
                    "Player {:?} as {:?} has too many tokens placed",
                    player, own_god.god_name
                ));
            }
        }

        if own_god.god_name == GodName::Hermes {
            if worker_count > 2 {
                return Err(format!(
                    "Player {:?} as Hermes can't have more than 2 workers",
                    player
                ));
            }
        }

        if own_god.god_name == GodName::Eros {
            if worker_count > 2 {
                return Err(format!(
                    "Player {:?} as Eros can't have more than 2 workers",
                    player
                ));
            }
        }

        if own_god.god_name == GodName::Castor {
            if worker_count > 2 {
                return Err(format!(
                    "Player {:?} as Castor can't have more than 2 workers",
                    player
                ));
            }

            if [GodName::Persephone, GodName::Harpies, GodName::Hypnus]
                .contains(&other_god.god_name)
            {
                if !(worker_count == 0 || worker_count == 2) {
                    return Err(format!(
                        "Player {:?} as Castor must have exactly 2 workers vs {:?}",
                        player, other_god.god_name,
                    ));
                }
            }
        }

        if own_god.god_name == GodName::Hydra {
            if worker_count > 11 {
                return Err(format!(
                    "Player {:?} has too many workers as hydra ({})",
                    player, worker_count
                ));
            }
        } else if worker_count > 4 {
            return Err(format!(
                "Player {:?} has too many workers ({})",
                player, worker_count
            ));
        }

        if own_god.is_hypnus() && oppo_count == 1 && other_god.god_name != GodName::Hydra {
            return Err("Can't play hypnus against a solo worker".to_owned());
        }

        if own_god.is_hypnus() && other_god.god_name == GodName::Artemis && oppo_count > 2 {
            return Err("Can't play hypnus against artemis with >2 workers".to_owned());
        }

        Ok(())
    }

    fn representation_err(&self, base_hash: HashType, gods: GodPair) -> Result<(), String> {
        self._validate_player(Player::One, gods[0])?;
        self._validate_player(Player::Two, gods[1])?;

        for h in 1..4 {
            if (self.height_map[h] & BitBoard::OFF_SECTION_MASK).is_not_empty() {
                return Err(format!(
                    "Unexpected bits in height map upper section: {}",
                    h
                ));
            }
        }

        for p in 0..2 {
            if (self.workers[p] & BitBoard::OFF_SECTION_MASK).is_not_empty() {
                return Err(format!("Unexpected bits in workers for player {}", p + 1));
            }
        }

        for h in 1..4 {
            let height = self.height_map[h] & BitBoard::MAIN_SECTION_MASK;
            let lower = self.height_map[h - 1] & BitBoard::MAIN_SECTION_MASK;

            if (height & !lower).is_not_empty() {
                for h in 0..4 {
                    eprintln!("{h}: {}", self.height_map[h]);
                }

                return Err(format!("Board has corrupted state on height {h}"));
            }
        }

        if self.hash != compute_hash_from_scratch_for_board(self, base_hash) {
            let diff = self.hash ^ compute_hash_from_scratch_for_board(self, base_hash);
            return Err(format!(
                "Hash mismatch: expected {:064b}, got {:064b} (diff: {:064b})",
                compute_hash_from_scratch_for_board(self, base_hash),
                self.hash,
                diff
            ));
        }

        Ok(())
    }

    pub fn get_worker_at(&self, square: Square) -> Option<Player> {
        if (self.workers[0] & BitBoard::as_mask(square)).is_not_empty() {
            Some(Player::One)
        } else if (self.workers[1] & BitBoard::as_mask(square)).is_not_empty() {
            Some(Player::Two)
        } else {
            None
        }
    }

    pub fn print_to_console(&self) {
        if let Some(winner) = self.get_winner() {
            eprintln!("Player {:?} wins!", winner);
        } else {
            eprintln!("Player {:?} to play", self.current_player);
        }

        for row in 0_usize..5 {
            let mut row_str = format!("{}", 5 - row);
            for col in 0_usize..5 {
                let pos = col + row * 5;
                let mask = 1 << pos;
                let height = self.get_height(pos.into());

                let is_1 = self.workers[0].0 & mask > 0;
                let is_2 = self.workers[1].0 & mask > 0;

                assert!(
                    !(is_1 && is_2),
                    "A square cannot have both players' workers"
                );

                let char = if is_1 {
                    "X"
                } else if is_2 {
                    "0"
                } else {
                    " "
                }
                .black();

                let elem = match height {
                    0 => char.on_white(),
                    1 => char.on_yellow(),
                    2 => char.on_blue(),
                    3 => char.on_green(),
                    4 => char.on_black(),
                    _ => panic!("Invalid Height: {}", height),
                };
                row_str = format!("{row_str}{elem}");
            }
            eprint!("{}", row_str);
            eprintln!()
        }
        eprintln!(" ABCDE");
    }

    pub fn get_positions_for_player(&self, player: Player) -> Vec<Square> {
        let workers_mask = self.workers[player as usize] & BitBoard::MAIN_SECTION_MASK;
        workers_mask.into_iter().collect()
    }

    pub fn as_basic_game_state(&self) -> FullGameState {
        FullGameState::new(
            self.clone(),
            [GodName::Mortal.to_power(), GodName::Mortal.to_power()],
        )
    }

    fn _flip_vertical_mut(&mut self, gods: GodPair) {
        self.height_map[0] = self.height_map[0].flip_vertical();
        self.height_map[1] = self.height_map[1].flip_vertical();
        self.height_map[2] = self.height_map[2].flip_vertical();
        self.height_map[3] = self.height_map[3].flip_vertical();
        self.workers[0] = self.workers[0].flip_vertical();
        self.workers[1] = self.workers[1].flip_vertical();
        self.god_data[0] = gods[0].get_flip_vertical_god_data(self.god_data[0]);
        self.god_data[1] = gods[1].get_flip_vertical_god_data(self.god_data[1]);
    }

    fn _flip_horizontal_mut(&mut self, gods: GodPair) {
        self.height_map[0] = self.height_map[0].flip_horizontal();
        self.height_map[1] = self.height_map[1].flip_horizontal();
        self.height_map[2] = self.height_map[2].flip_horizontal();
        self.height_map[3] = self.height_map[3].flip_horizontal();
        self.workers[0] = self.workers[0].flip_horizontal();
        self.workers[1] = self.workers[1].flip_horizontal();
        self.god_data[0] = gods[0].get_flip_horizontal_god_data(self.god_data[0]);
        self.god_data[1] = gods[1].get_flip_horizontal_god_data(self.god_data[1]);
    }

    fn _transpose_mut(&mut self, gods: GodPair) {
        self.height_map[0] = self.height_map[0].flip_transpose();
        self.height_map[1] = self.height_map[1].flip_transpose();
        self.height_map[2] = self.height_map[2].flip_transpose();
        self.height_map[3] = self.height_map[3].flip_transpose();
        self.workers[0] = self.workers[0].flip_transpose();
        self.workers[1] = self.workers[1].flip_transpose();
        self.god_data[0] = gods[0].get_flip_transpose_god_data(self.god_data[0]);
        self.god_data[1] = gods[1].get_flip_transpose_god_data(self.god_data[1]);
    }

    fn _flip_vertical_clone(&self, gods: GodPair) -> Self {
        let mut result = self.clone();
        result._flip_vertical_mut(gods);
        result
    }

    fn _flip_horz_clone(&self, gods: GodPair) -> Self {
        let mut result = self.clone();
        result._flip_horizontal_mut(gods);
        result
    }

    fn _transpose_clone(&self, gods: GodPair) -> Self {
        let mut result = self.clone();
        result._transpose_mut(gods);
        result
    }

    pub fn get_all_permutations<const INCLUDE_SELF: bool>(
        &self,
        gods: GodPair,
        base_hash: HashType,
    ) -> Vec<Self> {
        let horz = self._flip_horz_clone(gods);
        let vert = self._flip_vertical_clone(gods);
        let hv = horz._flip_vertical_clone(gods);
        let trans = self._transpose_clone(gods);
        let th = trans._flip_horz_clone(gods);
        let tv = trans._flip_vertical_clone(gods);
        let tvh = th._flip_vertical_clone(gods);

        let mut res = if INCLUDE_SELF {
            vec![self.clone(), horz, vert, hv, trans, th, tv, tvh]
        } else {
            vec![horz, vert, hv, trans, th, tv, tvh]
        };

        for board in &mut res {
            board.recalculate_internals(base_hash);
        }

        res
    }
}

impl Ord for BoardState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.current_player
            .cmp(&other.current_player)
            .then(self.height_map[0].cmp(other.height_map[0]))
            .then(self.height_map[1].cmp(other.height_map[1]))
            .then(self.height_map[2].cmp(other.height_map[2]))
            .then(self.height_map[3].cmp(other.height_map[3]))
            .then(self.workers[0].cmp(other.workers[0]))
            .then(self.workers[1].cmp(other.workers[1]))
            .then(self.god_data[0].cmp(&other.god_data[0]))
            .then(self.god_data[1].cmp(&other.god_data[1]))
    }
}

impl PartialOrd for BoardState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

#[cfg(test)]
mod tests {
    use crate::square::Square;

    #[test]
    fn test_serde_coord() {
        for position in 0_usize..25 {
            let coord = Square::from(position);
            let coord_str = serde_json::to_string(&coord).unwrap();
            let parsed_coord: Square = serde_json::from_str(&coord_str).unwrap();

            assert_eq!(coord, parsed_coord);
        }
    }
}
