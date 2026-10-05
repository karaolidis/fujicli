# Codegen

The pipeline entrypoint is
[`fujicodegen::generate`](../../crates/fujicodegen/src/lib.rs); from there each
`common/*::generate` function returns a `proc_macro2::TokenStream` that gets
formatted by `prettyplease` and written to disk.

The mechanical contract every emitter follows:

- Take the relevant slice of the AST (e.g. `&BTreeMap<String, FujiOption>`,
  `&BTreeMap<String, Camera>`).
- Return a `TokenStream` containing self-contained Rust.
- Reference cross-module types through their canonical path
  (`crate::generated::options::...`, `crate::ptp::option::SimulationSetting`,
  etc.); never assume the module is `use`'d.

## Options

[`common/options/`](../../crates/fujicodegen/src/common/options/) emits one
block per option. The dispatch is in `options/mod.rs`:

| `kind`    | `encoding.kind` | Emitter                     |
| --------- | --------------- | --------------------------- |
| `enum`    | `lookup`        | `options/enum/mod.rs`       |
| `integer` | `lookup`        | `options/integer/lookup.rs` |
| `integer` | `raw` / `scale` | `options/integer/scaled.rs` |
| `float`   | `lookup`        | `options/float/lookup.rs`   |
| `float`   | `raw` / `scale` | `options/float/scaled.rs`   |
| `string`  | `raw`           | `options/string/mod.rs`     |

### Repr Resolution

Numeric and enum wire types are always `i16` or `u16` (Fuji's convention).
[`options/common.rs`](../../crates/fujicodegen/src/common/options/common.rs)
picks signedness from the observed range:

- All values fit in `[0, u16::MAX]` -> `u16`.
- Any negative value, all values fit in `[i16::MIN, i16::MAX]` -> `i16`.
- Span crosses both -> emit an error
  (`wire value range fits neither
  i16 nor u16`).

The 32-bit profile codec lifts these to `i32`/`u32` via
[`ConversionProfileField`](../../crates/fujicore/src/ptp/option.rs).

### Shapes

**Enums and integer-lookup**: `#[repr(uN/iN)] enum Name { ... }` with explicit
discriminants set to the canonical wire value. Variants come with
`from_nearest_*` snapping (lookup ints/floats), `try_from_wire` (canonical +
alternate wire values), `Display` (variant name), `FromStr` with alias matching,
serde, PTP serde, `SimulationSetting`, and `ConversionProfileField`.

**Integer-scaled**: newtype `pub struct Name(i16/u16)`. Carries inherent consts
`MIN/MAX/STEP/SCALE` (logical) and `RAW_MIN/RAW_MAX/RAW_STEP` (raw).
`TryFrom<i32>` validates range + step, then multiplies by `SCALE` before
storing. `From<Self> for i32` divides back.

**Float-scaled**: newtype `pub struct Name(i16)`. Same shape as integer-scaled
but with `f32` logical bounds. Step alignment uses `% STEP != 0.0` - _this is
the known sharp edge_; for non-power-of- two steps consider using `lookup`
encoding instead.

**String**: newtype `pub struct Name(String)`. Validates min/max length in
`FromStr`. No `Copy` (obviously).

### Aliases and Parsing

`FromStr` runs the input through
[`crate::input::CleanAlphanumeric::clean`](../../crates/fujicore/src/input/).
Drops whitespace, punctuation, case-folds. So `--white-balance "As Shot"`,
`--white-balance as_shot`, and `--white-balance ASSHOT` all collide into the
same key. If nothing matches, `Choices::closest` runs Levenshtein over the known
aliases and the error tells the user "Did you mean 'auto'?".

## Cameras

[`common/cameras/mod.rs`](../../crates/fujicodegen/src/common/cameras/mod.rs)
sorts cameras by `(generation, id)` for stable output and emits, per camera:

- `pub struct XT5;` - zero-sized; carries only the type.
- `pub const C_X_T5: SupportedCamera { name, vendor, product,
  camera_factory }`
  - registry entry.
- `impl CameraBase for XT5` - overrides `chunk_size`, returns the registry
  constant, and conditionally overrides `as_backup_manager`,
  `as_simulation_parser`, `as_simulation_manager`, `as_render_manager` based on
  which feature blocks the camera declares.

A `pub const SUPPORTED: &[SupportedCamera] = &[C_X_T5, ...]` is emitted at the
end. The runtime's `Camera::probe`/`open_with` scans this list by
`(vendor, product)`.

## Simulations

[`common/simulations/`](../../crates/fujicodegen/src/common/simulations/) emits
two things:

**`SimulationBase`**
([base.rs](../../crates/fujicodegen/src/common/simulations/base.rs)) - the union
of every simulation field across every simulation-capable camera. All fields are
`Option<...>`. This is the partial users build via CLI args / JSON; cameras
consume it via `try_update_from`.

**`<Camera>Simulation`**
([camera.rs](../../crates/fujicodegen/src/common/simulations/camera.rs)) -
per-camera struct with typed fields, `apply_transformations`,
`emit_warnings_and_infos`, `solve`, `try_update_from`, `name`, serde,
`From<&Self>` for `SimulationBase`, `TryFrom<SimulationBase>`, `Display`, plus
the `Simulation` trait impl (`try_pull` / `try_push` talking to PTP),
`CameraSimulationParser` impl (JSON serialize/deserialize), and
`CameraSimulationManager` impl (`get_simulation`, `update_simulation`,
`set_simulation`, `custom_settings_slots`).

`try_update_from` is the key method:

```
1. partial_profile = take the SimulationBase, project into Self fields
2. partial_profile.apply_transformations()
3. pin = the set of field ids that were Some in partial_profile (the
   user's explicit choices, post-transformation)
4. candidate = self.clone()
5. for each field set in partial_profile: candidate.<field> = Some(value)
6. candidate.apply_transformations()
7. candidate.solve(&pin)   // see analyses
8. candidate.emit_warnings_and_infos()
9. *self = candidate
```

`solve` is allowed to clear any field **not** in `pin`. That's how the engine
respects "I set X explicitly; repair around it" without needing extra
annotations.

The render path additionally snapshots `original = self.clone()` before the
merge and threads it through `solve` and `emit_warnings_and_infos`; see
[analyses / scope](analyses.md#scope).

`try_pull` reads each setting in topological read order (see
[analyses / presence DAG](analyses.md#the-presence-dag)); for each field with a
derived gate, the read is wrapped in `if gate { read }
else { None }`.
`try_push` writes every `Some` field in write order.

## Renders

[`common/renders/`](../../crates/fujicodegen/src/common/renders/) mirrors
simulations but with a wire codec instead of per-setting PTP calls.

**`RenderBase`** carries the union of all render fields plus a `merge(overlay)`
and a `try_update_from(&SimulationBase)` that copies overlapping settings from a
simulation into the render base.

**`<Camera>RenderProfile`** is the per-camera struct. It implements:

- The same inherent methods as simulations: `apply_transformations`,
  `emit_warnings_and_infos`, `solve`, `try_update_from`.
- `PtpSerialize` - writes
  `[i16 n_props][hex profile_code
  ExactString][padding][i32 * n_props]` in
  declaration order. For ref-fields, uses `ConversionProfileField`. For inline
  fields, writes a raw `i32` (defaulting to 0 if unset).
- `PtpDeserialize` - reads the same header, then `i32` fields in declaration
  order. Each field is converted to its typed form _in topological convert
  order_ (so gated fields evaluate their gates against already-converted
  fields). Inverses run last to lift wire-level multi-field encodings back to
  user-facing ones.
- `CameraRenderManager` - orchestrates the in-camera render pipeline: send_image
  -> get current profile -> try_update_from(partial) -> set profile ->
  render_image.

A hard-coded constant
[`RENDER_HEADER_PADDING`](../../crates/fujicodegen/src/common/renders/camera.rs)
holds the 0x1EE-byte padding observed on the X-T5; it'll need to move into the
camera spec when a render-capable body with a different padding is added.

## CLI

[`common/cli/`](../../crates/fujicodegen/src/cli/) emits two
`clap::Args`-deriving structs:

- **`SimulationArgs`** - every option that some simulation-capable camera uses
  and that doesn't have `codegen.skip_args`. Each field is `Option<OptionType>`
  with `#[clap(long, allow_hyphen_values(true))]`.
- **`RenderArgs`** - same idea but for render fields. Inline fields (no `ref`)
  are not exposed; users have no business setting padding.

Both come with `From<Args> for *Base` impls that map field-by-field, adding
`..Default::default()` only when there are union members the args struct doesn't
cover.

Plus `SIMULATION_PROP_CODES: &[u16]` - the union of PTP property codes touched
by every simulation setting, used by the runtime when polling for property
changes.

## Utilities

[`util/dag.rs`](../../crates/fujicodegen/src/util/dag.rs) - Kahn's algorithm
with a min-heap keyed on declaration index. Among all valid topological orders
we emit the one closest to declaration order. This matters because generated
code preserves the order an FML author wrote settings in, only perturbing where
read-gating demands it.

[`util/ident.rs`](../../crates/fujicodegen/src/util/ident.rs) - safe Rust
identifiers. Digit-leading idents get an `X` prefix (`7728x5152` ->
`X7728x5152`). Numeric-lookup variants get `Plus`/`Minus`/`Zero` (`Plus0_3`,
`Minus4`, `Zero`).

[`util/multiset.rs`](../../crates/fujicodegen/src/util/multiset.rs) - multiset
`eq` / `hash` / `subset` used everywhere conjunctions compare
order-insensitively (alias matching, conjunction equality in DNF).
