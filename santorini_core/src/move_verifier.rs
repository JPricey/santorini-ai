use crate::gods::GameStateWithAction;
use crate::player::Player;
use crate::square::Square;

/// Fluent predicate builder for asserting properties of move generation results.
/// Compose predicates with builder methods, then assert with a terminal method.
///
/// ```ignore
/// MoveVerifier::new()
///     .is_winner(Player::One)
///     .any(&next_states);
/// ```
pub struct MoveVerifier {
    predicates: Vec<Box<dyn Fn(&GameStateWithAction) -> bool>>,
}

impl MoveVerifier {
    pub fn new() -> Self {
        Self {
            predicates: Vec::new(),
        }
    }

    pub fn with_p1_worker_at(mut self, square: Square) -> Self {
        let mask = square.to_board();
        self.predicates
            .push(Box::new(move |s| (s.state.board.workers[0] & mask).is_not_empty()));
        self
    }

    pub fn with_p2_worker_at(mut self, square: Square) -> Self {
        let mask = square.to_board();
        self.predicates
            .push(Box::new(move |s| (s.state.board.workers[1] & mask).is_not_empty()));
        self
    }

    pub fn without_p1_worker_at(mut self, square: Square) -> Self {
        let mask = square.to_board();
        self.predicates
            .push(Box::new(move |s| (s.state.board.workers[0] & mask).is_empty()));
        self
    }

    pub fn without_p2_worker_at(mut self, square: Square) -> Self {
        let mask = square.to_board();
        self.predicates
            .push(Box::new(move |s| (s.state.board.workers[1] & mask).is_empty()));
        self
    }

    pub fn is_winner(mut self, player: Player) -> Self {
        self.predicates
            .push(Box::new(move |s| s.state.board.get_winner() == Some(player)));
        self
    }

    pub fn no_winner(mut self) -> Self {
        self.predicates
            .push(Box::new(|s| s.state.board.get_winner().is_none()));
        self
    }

    pub fn with_height_at(mut self, square: Square, height: usize) -> Self {
        self.predicates
            .push(Box::new(move |s| s.state.board.get_height(square) == height));
        self
    }

    fn matches(&self, state: &GameStateWithAction) -> bool {
        self.predicates.iter().all(|p| p(state))
    }

    fn count_matches(&self, states: &[GameStateWithAction]) -> usize {
        states.iter().filter(|s| self.matches(s)).count()
    }

    /// Assert that at least one state matches all predicates.
    pub fn any(self, states: &[GameStateWithAction]) {
        let matched = self.count_matches(states);
        assert!(
            matched > 0,
            "Expected at least one matching state, found 0 out of {}",
            states.len()
        );
    }

    /// Assert that no state matches all predicates.
    pub fn none(self, states: &[GameStateWithAction]) {
        let matched = self.count_matches(states);
        assert!(
            matched == 0,
            "Expected no matching states, found {} out of {}",
            matched,
            states.len()
        );
    }

    /// Assert that every state matches all predicates.
    pub fn all(self, states: &[GameStateWithAction]) {
        let total = states.len();
        let matched = self.count_matches(states);
        assert!(
            matched == total,
            "Expected all {} states to match, but only {} did",
            total, matched
        );
    }

    /// Assert that exactly `n` states match all predicates.
    pub fn count(self, states: &[GameStateWithAction], n: usize) {
        let matched = self.count_matches(states);
        assert!(
            matched == n,
            "Expected exactly {} matching states, found {} out of {}",
            n, matched, states.len()
        );
    }
}
