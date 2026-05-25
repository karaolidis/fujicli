# Fuji Modelling Language

**FML** is the CUE-based schema in [`fml/`](../../fml/) that describes every
option, generation, and camera `fujicli` knows about. The
[`codegen`](../../crates/codegen/) crate consumes its JSON export at build time
and emits Rust types and trait impls under `src/lib/generated/` (gitignored;
regenerated on every build).

For users adding a camera, this is the only thing you edit. For users extending
the schema language itself, see [internals](../internals/codegen.md).

## Why CUE

CUE gives us three properties that drive the design:

1. **Constraints unify.** A camera's `simulation.settings` can reference
   `_generation._simulation.settings` and overlay extras, and CUE will reject
   the result at export if a setting points to an option that doesn't exist or
   violates its rules.
2. **Strong typing across the spec.** Predicate leaves carry typed refs
   (`_ids.i`, `_ids.f`, `_ids.s`, `_ids.e`) so you can't write
   `{ref: "image_size", lt: 5}` against an enum.
3. **Composability.** Cameras inherit a generation's defaults via a single ref;
   updating the generation propagates everywhere.

## Layout

```
fml/
  main.cue        # Definition root
  grammar.cue     # Shared predicate / assignment grammar
  option.cue      # Option types + every option definition
  generation.cue  # Generation-level capability presets
  camera.cue      # Per-model overrides + feature blocks
```

All files belong to the `fml` package. There is no entry-point file;
`cue export ./fml --out json` materializes the whole package into one JSON
document.

## How to Read the Rest of These Pages

- [Options](options.md) - every setting the camera understands. Kinds
  (`integer`, `float`, `string`, `enum`), rules, encodings. Start here when
  adding a new property.
- [Generations](generations.md) - sharing settings and rules across many
  cameras. Useful when you've reversed something on one body and want the rest
  of the family to inherit it.
- [Cameras](cameras.md) - per-model `spec` block. USB IDs, feature toggles, the
  simulation and render lists.
- [Grammar](grammar.md) - predicates and assignments. The leaf-level vocabulary
  every rule and transformation is built from.
- [Rules and Transformations](rules.md) - the _semantics_: how a validation rule
  becomes a runtime check, how a transformation rewrites the value space, and
  what guarantees the generated `solve` / read-gating actually offers.

## A Worked Example

```cue
options: noise_reduction: #Option & {
    spec: {
        name: "High ISO Noise Reduction"
        kind: "integer"
        rules: {min: -4, max: 4, step: 1}
        encoding: {
            prop_code: 0xD1A1
            kind:      "lookup"
            spec: values: {
                "4":  0x5000
                "3":  0x6000
                "2":  0x0000
                "1":  0x1000
                "0":  0x2000
                "-1": 0x3000
                "-2": 0x4000
                "-3": 0x7000
                "-4": 0x8000
            }
        }
    }
}
```

This is an `integer` option called `noise_reduction`. Logical values range
`[-4, 4]`, step 1.

On the wire the camera uses non-monotonic codes that don't follow a clean scale,
so we use `lookup` encoding with explicit key -> wire mappings.

`prop_code: 0xD1A1` is the PTP property code: present means the option is
readable/writable as a top-level PTP property, absent means it's used only as a
render-profile slot.

Codegen produces an enum with `Minus4` ... `Plus4` variants, `from_nearest_i32`
for snapping, and the PTP serde impls.

The same option then appears in `fml/generation.cue` under
`x_trans_iv._simulation.settings`, which means every X-Trans IV+ body that opts
into the generation's settings will read/write `noise_reduction` as part of its
simulation profile.
