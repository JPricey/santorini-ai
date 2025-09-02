# SANTORINI AI

Santorini AI is a game engine for the board game Santorini, built using negamax search over a NNUE evaluation.  
Santorini AI is a full implementation of Mortal & all Simple God powers, with more powers under development.

This project may be interfaced with in a few ways:
- Play against the AI on the web, [here](https://jpricey.github.io/god-game/)
- Run the native analysis engine, under the ui package
- Run the native UCI process, under the uci package

## Running locally
This project requires nightly rust:
`rustup install nightly-2025-07-26`

Run the analysis engine UI:
`cargo run -p ui -r`

Run the standalong UCI, for use with a different UI:
`cargo run -p uci -r`

## Acklowledgements
Big thanks to these other projects for inspiration and tooling:  
[viridithas](https://github.com/cosmobobak/viridithas)  
[bullet](https://github.com/jw1912/bullet)
