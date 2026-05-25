# Architecture

The project is split into a **build-time** pipeline that turns FML into Rust and
a **runtime** that consumes the generated code. The runtime is small, generic,
and dispatches everything through traits the codegen implements.

- [Codegen](codegen.md) - what each emitter produces (option types, camera
  registry, simulation/render structs, CLI args).
- [Analyses](analyses.md) - the semantic passes between AST and emission: DNF
  normalization, alias substitution, the presence DAG, the repair walk, and
  inverse transformations.
- [Runtime](runtime.md) - how the generated modules plug into the trait-based
  runtime (`CameraBase`, `Simulation`, `CameraRenderManager`,
  `SimulationSetting`, `ConversionProfileField`).

## Build Pipeline

[build.rs](../../build.rs) is the entrypoint. It:

1. Tells cargo to rerun if `fml/` or `crates/codegen/` change.
2. Shells out to `cue export ./fml --out json`. If `cue` isn't on `PATH`, the
   error tells the user how to install it.
3. Passes the JSON to `codegen::generate(json, &generated)`.

`codegen::generate` (see [lib.rs](../../crates/codegen/src/lib.rs)):

```
options     -> src/lib/generated/options.rs      (~3.4 KLOC for current schema)
cameras     -> src/lib/generated/cameras.rs      (one ZST + registry entry per camera)
simulations -> src/lib/generated/simulations.rs  (SimulationBase + per-camera structs)
renders     -> src/lib/generated/renders.rs      (RenderBase + per-camera profiles)
cli         -> src/lib/generated/cli.rs          (SimulationArgs + RenderArgs + PROP_CODES)
mod         -> src/lib/generated/mod.rs          (module roots)
```

Output is formatted through `prettyplease` before being written, so any
diagnostic dump of the file is human-readable.

`src/lib/generated/` is gitignored. Builds wipe and rewrite it; `cargo build` on
a fresh checkout always regenerates from `fml/`. Don't edit the files directly,
changes are lost on the next build.

## Why the Analyses Live in Their Own Module

`schema/` is the layer that does anything interesting. The emitters in `common/`
are mostly mechanical: turn a typed AST node into a TokenStream. The cleverness

- DNF normalization, alias substitution, the presence DAG, repair synthesis,
  inverse detection - lives in `schema/` and is unit-tested independently of the
  emitters.

This separation means changing the _language_ (add a new predicate kind, a new
transformation shape) only touches `ast/` + `schema/`, not the emitters.
Changing the _output_ (new trait, new derive) only touches `common/`, not the
analyses.

Read the next two pages in order: [codegen](codegen.md) for the mechanical
layer, then [analyses](analyses.md) for the clever bits.
