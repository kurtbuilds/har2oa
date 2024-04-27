set positional-arguments
set dotenv-load := true

@help:
    just --list --unsorted

run *ARGS:
    cargo run -- "$@"

test *ARGS:
    cargo test -- "$@"

pick-har:
    cargo run --bin pick-har

studiodesigner:
    cargo run -- generate data/app.studiodesigner.com.har --cookie sessid

install:
    cargo install --path .

check:
    cargo check
