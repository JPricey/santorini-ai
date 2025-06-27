/// TODO: this
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SprtResult {
    /// Not enough data to make a conclusion
    Continue,
    /// H0 (null hypothesis) is accepted - no improvement detected
    AcceptH0,
    /// H1 (alternative hypothesis) is accepted - improvement detected
    AcceptH1,
}

/// Performs Sequential Probability Ratio Test (SPRT) for comparing two actors
pub struct SprtCalculator {
    // Game results
    wins: u32,
    losses: u32,
    draws: u32,
    
    // SPRT parameters
    elo0: f64, // H0: Elo difference is elo0 or less
    elo1: f64, // H1: Elo difference is elo1 or more
    alpha: f64, // Type I error (false positive) probability
    beta: f64,  // Type II error (false negative) probability
}

impl SprtCalculator {
    /// Creates a new SPRT calculator with specified parameters
    pub fn new(elo0: f64, elo1: f64, alpha: f64, beta: f64) -> Self {
        Self {
            wins: 0,
            losses: 0,
            draws: 0,
            elo0,
            elo1,
            alpha,
            beta,
        }
    }
    
    /// Creates a new SPRT calculator with default parameters
    pub fn default_test() -> Self {
        // Common default values for Elo testing
        Self::new(0.0, 10.0, 0.05, 0.05)
    }
    
    /// Adds a win for the first player
    pub fn add_win(&mut self) {
        self.wins += 1;
    }
    
    /// Adds a loss for the first player (win for second player)
    pub fn add_loss(&mut self) {
        self.losses += 1;
    }
    
    /// Adds a draw
    pub fn add_draw(&mut self) {
        self.draws += 1;
    }
    
    /// Updates results based on game outcome
    pub fn add_result(&mut self, first_player_score: f64) {
        if first_player_score == 1.0 {
            self.add_win();
        } else if first_player_score == 0.0 {
            self.add_loss();
        } else {
            self.add_draw();
        }
    }
    
    /// Gets total number of games
    pub fn total_games(&self) -> u32 {
        self.wins + self.losses + self.draws
    }
    
    /// Calculates the log-likelihood ratio
    fn llr(&self) -> f64 {
        let games = self.total_games() as f64;
        if games == 0.0 {
            return 0.0;
        }
        
        // Convert Elo difference to probability
        let p0 = 1.0 / (1.0 + 10.0_f64.powf(-self.elo0 / 400.0));
        let p1 = 1.0 / (1.0 + 10.0_f64.powf(-self.elo1 / 400.0));
        
        // Calculate actual score
        let wins = self.wins as f64;
        let losses = self.losses as f64;
        let draws = self.draws as f64;
        
        // Draw value is 0.5 for both players
        let score = wins + 0.5 * draws;
        
        // Calculate log-likelihood ratio
        score * (p1.ln() - p0.ln()) + 
            (games - score) * ((1.0 - p1).ln() - (1.0 - p0).ln())
    }
    
    /// Runs the SPRT test and returns the current result
    pub fn get_result(&self) -> SprtResult {
        let llr = self.llr();
        
        // Decision boundaries
        let a = (1.0 - self.beta).ln() - self.alpha.ln();
        let b = self.beta.ln() - (1.0 - self.alpha).ln();
        
        if llr >= a {
            SprtResult::AcceptH1 // Improvement detected
        } else if llr <= b {
            SprtResult::AcceptH0 // No improvement detected
        } else {
            SprtResult::Continue // Need more data
        }
    }
    
    /// Returns true if a conclusion has been reached
    pub fn has_conclusion(&self) -> bool {
        matches!(self.get_result(), SprtResult::AcceptH0 | SprtResult::AcceptH1)
    }
    
    /// Gets current statistics as a string
    pub fn stats_string(&self) -> String {
        format!("W: {}, L: {}, D: {}, Total: {}", 
                self.wins, self.losses, self.draws, self.total_games())
    }
}

impl fmt::Display for SprtCalculator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total = self.total_games();
        if total == 0 {
            return write!(f, "No games recorded");
        }
        
        let win_rate = 100.0 * (self.wins as f64) / (total as f64);
        let result = self.get_result();
        
        write!(f, "Games: {} (W: {}, L: {}, D: {}), Win rate: {:.2}%, Status: {:?}",
               total, self.wins, self.losses, self.draws, win_rate, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_functionality() {
        let mut sprt = SprtCalculator::new(0.0, 50.0, 0.05, 0.05);
        
        // Initially should have no conclusion
        assert_eq!(sprt.get_result(), SprtResult::Continue);
        
        // Add some results
        for _ in 0..20 {
            sprt.add_win();
        }
        for _ in 0..10 {
            sprt.add_loss();
        }
        for _ in 0..5 {
            sprt.add_draw();
        }
        
        assert_eq!(sprt.total_games(), 35);
        assert!(sprt.llr() > 0.0); // Should be positive with more wins
    }
}
```

Now we need to add this module to the library:

[file:lib.rs](santorini_core/src/lib.rs)
```rust
pub mod sprt;
```

This SPRT calculator allows you to:
1. Track wins, losses, and draws between two agents
2. Set confidence parameters (elo0, elo1, alpha, beta)
3. Check if there's statistically significant evidence that one agent is stronger than another
4. Get detailed statistics about the ongoing comparison

You can use it like this:

```rust
let mut sprt = SprtCalculator::default_test();

// Play games and record results
sprt.add_win();   // First player wins
sprt.add_loss();  // Second player wins
sprt.add_draw();  // Draw

// Continue until a conclusion is reached
while !sprt.has_conclusion() {
    // play more games and record results
}

// Check the final result
match sprt.get_result() {
    SprtResult::AcceptH0 => println!("No significant improvement detected"),
    SprtResult::AcceptH1 => println!("Improvement confirmed"),
    SprtResult::Continue => println!("No conclusion yet"),
}
