#!/bin/bash

sudo apt install -y unzip build-essential
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source /root/.cargo/env
cargo run -p datagen -r -- --p1 stymphalians
