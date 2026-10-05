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

[crates/fujicore/build.rs](../../crates/fujicore/build.rs) is the entrypoint.
It:

1. Tells cargo to rerun if `fml/` or `crates/fujicodegen/` change.
2. Shells out to `cue export ./fml --out json`. If `cue` isn't on `PATH`, the
   error tells the user how to install it.
3. Passes the JSON to `fujicodegen::generate(json, &generated)`.

`fujicodegen::generate` (see [lib.rs](../../crates/fujicodegen/src/lib.rs)):

```
options     -> crates/fujicore/src/generated/options.rs      (~3.4 KLOC for current schema)
cameras     -> crates/fujicore/src/generated/cameras.rs      (one ZST + registry entry per camera)
simulations -> crates/fujicore/src/generated/simulations.rs  (SimulationBase + per-camera structs)
renders     -> crates/fujicore/src/generated/renders.rs      (RenderBase + per-camera profiles)
cli         -> crates/fujicore/src/generated/cli.rs          (SimulationArgs + RenderArgs + PROP_CODES)
mod         -> crates/fujicore/src/generated/mod.rs          (module roots)
```

Output is formatted through `prettyplease` before being written, so any
diagnostic dump of the file is human-readable.

`crates/fujicore/src/generated/` is gitignored. Builds wipe and rewrite it;
`cargo build` on a fresh checkout always regenerates from `fml/`. Don't edit the
files directly, changes are lost on the next build.

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
