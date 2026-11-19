# Contributing

Issues and pull requests are welcome on the
[GitHub mirror](https://github.com/karaolidis/fujicli). The canonical repository
is on my personal [Gitea](https://git.karaolidis.com/karaolidis/fujicli), which
does not take registrations. Pull requests are applied there by hand, sometimes
with changes, keeping you as the author or crediting you in the commit, and then
closed on GitHub.

Most changes are to the camera schema under `fml/`; see `fujicli-fml(5)` for the
language and `fujicli(7)` for adding a camera.

## Setup

`nix develop` provides the Rust toolchain and CUE. Without Nix, install a
nightly Rust toolchain, [CUE](https://cuelang.org/), and the `libusb-1.0`
headers yourself.

## Checks

CI runs:

```sh
cargo check --all-features --all-targets --workspace
cargo fmt --all --check
cargo clippy --all-features --all-targets --workspace -- -D warnings
cargo test --all-features --all-targets --workspace
```

Clippy's `all`, `pedantic`, and `nursery` groups are enabled for the workspace,
so every warning fails CI, and `unwrap()` is denied.

Format with `nix fmt`, which runs rustfmt, nixfmt, and shellcheck through
treefmt.

## Testing

The tests cover the analyses and the generated code. Nothing drives a real
camera, so test changes that touch the camera by hand on one, and attach the
`-vvv` output to the pull request.
