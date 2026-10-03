# Contributing

## Prerequisites

- `git`. The PSoXide SDK is vendored under `psoxide/` — there is **no submodule
  to initialise**. Check `ls psoxide/sdk` before building.
- The Rust nightly pinned in [`rust-toolchain.toml`](rust-toolchain.toml);
  `rustup` installs it automatically. It needs `rust-src` and `llvm-tools` for
  the `mipsel-sony-psx` cross build.
- A host C/C++ compiler, Python 3, and `binutils-mipsel-linux-gnu` for linking.

## Build

The cross build is configured by `game/.cargo/config.toml`, which Cargo only
reads from the **current directory**. Always build the game from inside `game/`:

```sh
cd game && cargo build --release
```

Running `cargo build --manifest-path game/Cargo.toml` from the repository root
fails, because the target triple is not picked up.

| Target | Effect |
|---|---|
| `make test` | Run the host-side logic suite |
| `make exe` | Build the bare-metal PS1 executable |
| `make disc` | Master `dist/plattypus.{bin,cue}` |
| `make iso` | Master a cooked `dist/plattypus.iso` |
| `make run` | Run the disc in an emulator |
| `make web` | Serve the Web Arcade build |

## Tests

`make test` runs the host-side suite (`tools/test_game_logic/`). It exercises
the **real** level generators, save format and rank evaluator out of
`plattypus-core` — not copies of them. If you need to change behaviour that the
suite asserts on, change the production code and let the test follow.

The suite includes exhaustive sweeps that are slow but valuable: every
single-bit checksum flip, and a full 4-D sweep of the codename evaluator.

## Lint and format

CI gates all four crates. Run the same checks locally before pushing:

```sh
for m in game crates/plattypus-core tools/test_game_logic video_bench; do
  cargo fmt --manifest-path "$m/Cargo.toml" --all
  cargo fmt --manifest-path "$m/Cargo.toml" --all -- --check
done

cargo clippy --offline --manifest-path crates/plattypus-core/Cargo.toml \
  --all-targets -- -D warnings
cargo clippy --offline --manifest-path tools/test_game_logic/Cargo.toml \
  --all-targets -- -D warnings
```

The `psoxide/` tree is vendored SDK code; its formatting is upstream's concern,
so the CI gate covers this project's four crates only.

## Conventions

- `#![no_std]` bare-metal Rust. No panics, no `unwrap`/`expect` in `game/src/`
  reachable before `main` returns — there is no allocator to unwind into.
- No `println!` outside `psx_rt::tty`.
- Prefer `saturating_sub`/`saturating_add` over unchecked arithmetic on
  gameplay counters.
- When you touch a magic number that encodes a hardware limit, name it and put a
  bound on it in a `const _: () = assert!(...)`.

## Licence

Contributions are accepted under GPL-2.0-or-later, matching this repository.
Third-party assets must be recorded in [`ASSETS.md`](ASSETS.md) with their
origin and licence before they are committed.