# Rust Solitaire

A lightweight browser Klondike solitaire game: Rust owns the game state and rules, the browser paints the board on one canvas, and a tiny dependency-free Rust server serves the static files.

## Build

```sh
make build
```

This builds the WASM game engine, copies it to `dist/solitaire.wasm`, and builds the server binary.

## Run

```sh
make run
```

Open `http://<raspberry-pi-ip>:3021/`.

To bind a different address or port:

```sh
SOLITAIRE_ADDR=127.0.0.1:3000 cargo run --release
```

By default the server writes scores to `leaderboard.tsv` and the game counter to `play-count.txt` next to it.
Those files are blocked from direct static file requests; use explicit paths outside the served root if you want them kept fully separate from public assets.
For a Raspberry Pi service, set explicit paths so systemd does not depend on its launch directory:

```sh
SOLITAIRE_ROOT=/home/pi/solitaire \
SOLITAIRE_LEADERBOARD_FILE=/home/pi/solitaire/leaderboard.tsv \
SOLITAIRE_PLAY_COUNT_FILE=/home/pi/solitaire/play-count.txt \
SOLITAIRE_ADDR=0.0.0.0:3021 \
./target/release/solitaire
```

## Test

```sh
make test
```
# solitaire
