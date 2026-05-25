# Contributing

Send contributions on [GitHub](https://github.com/karaolidis/fujicli). A
self-hosted [Gitea](https://git.karaolidis.com/karaolidis/fujicli) instance is
used for the author's own workflow; merges may happen through it, but
contributors don't need to interact with it.

## Workflow

No strict process; just be sane.

- Fork on GitHub and open a PR.
- A short note in the description (what, why) helps.
- `cargo build` and `cargo test --workspace` should pass.
- Run the project formatter if you have Nix (`nix fmt`); otherwise `cargo fmt`
  covers Rust.

## Where Things Live

- `fml/` - the CUE schema. **Most contributions go here.** Camera definitions,
  option types, validation rules.
- `crates/fujicodegen/` - the build-time crate that turns FML JSON into Rust.
  Touch this only if you're extending the schema language or changing what the
  generated code looks like.
- `crates/ptp-cursor/`, `crates/ptp-macro/` - PTP wire-format helpers and derive
  macros. Touch when adding new low-level types.
- `crates/fujicore/` - the runtime library: USB, PTP transport, feature traits,
  image-side helpers. `crates/fujicore/src/generated/` is the codegen output;
  it's gitignored and rebuilt on every `cargo build`.
- `crates/fujicli/` - the CLI front-end binary.
- `support/` - out-of-band scripts used during reversing.

## Common Contribution Types

| What you want to do                              | Where                                                                      |
| ------------------------------------------------ | -------------------------------------------------------------------------- |
| Add a new camera                                 | [adding-cameras.md](adding-cameras.md)                                     |
| Confirm a `?` in the support table               | Open an issue; see [support](../users/support.md)                          |
| Add or correct a film-simulation alias / variant | `fml/option.cue`; see [fml/options](../fml/options.md)                     |
| Add a new validation rule                        | `fml/camera.cue` or `fml/generation.cue`; see [fml/rules](../fml/rules.md) |
| Reverse a render profile                         | [reversing.md](reversing.md)                                               |
| Extend the codegen language                      | `crates/fujicodegen/`; see [internals](../internals/README.md)             |

## Testing

- `cargo test --workspace` runs the codegen unit tests (DNF, alias, presence
  DAG, repair output, predicate compiler).
- The build itself is a smoke test for any FML change: the codegen crate parses
  the JSON, runs the analyses, and the resulting Rust has to compile.
- There is no integration test that drives a real camera; for now, manual `-vvv`
  runs against a physical X-T5 are the gold standard. If you confirm a feature
  works on another body, attach the `-vvv` output to the PR.

## Code Style

- Rust: `cargo fmt` (uses [rustfmt.toml](../../rustfmt.toml)). No panicking
  outside `expect()` calls on invariants the CUE schema enforces.
- CUE: keep field ordering consistent with the existing files (`id` first, then
  `spec`, then `codegen?`). The schema itself enforces most invariants. If a CUE
  error is opaque, ask in the PR.

## Licensing

By contributing you agree your changes ship under the project's existing license
(see [LICENSE](../../LICENSE)).
