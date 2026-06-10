# Options

An _option_ is one typed scalar the camera knows about - a setting, slider, or
enum. Options are defined in [`fml/option.cue`](../../fml/option.cue) and
referenced everywhere else by id.

```cue
options: <id>: #Option & {
    spec:     <Spec>
    codegen?: <Codegen>
}
```

`<id>` is a stable identifier. It's a `snake_case` Rust ident; CUE guarantees
uniqueness across the map. The id is what `ref` fields point at throughout the
schema.

## Kinds

Every option declares a `kind`. The four kinds drive both validation and the
kind of Rust type that gets emitted.

| `kind`    | Logical type | Generated Rust shape                       |
| --------- | ------------ | ------------------------------------------ |
| `integer` | `i32`        | newtype `struct Name(i16/u16)` _or_ `enum` |
| `float`   | `f32`        | newtype `struct Name(i16)` _or_ `enum`     |
| `string`  | `String`     | newtype `struct Name(String)`              |
| `enum`    | named symbol | `#[repr(u16/i16)] enum`                    |

Which of the two shapes a numeric option gets depends on its `encoding`:

- `raw` / `scale` -> newtype struct (logical value is dense within the range).
- `lookup` -> enum (logical value is one of a finite set).

## Rules

`rules` constrain the logical (user-facing) value. They drive parsing, range
checks, and the bounds the generated `MIN`/`MAX`/`STEP` consts expose.

```cue
// integer
rules: {min?: int, max?: int, step?: int}

// float
rules: {min?: float, max?: float, step?: float}

// string
rules: {min_length?: uint, max_length?: uint}

// enum
rules: {variants: [...{id, name, aliases: [...]}]}
```

`variants[*].id` is what `ref` predicates name. `variants[*].name` is the
human-readable label (`Display`). `variants[*].aliases` is the list of strings
accepted at parse time, in addition to the id, so a user can type
`--white-balance auto`, `--white-balance Auto`, or `--white-balance temp` (and
get the closest-match suggestion if they typo).

Enum ids and variant aliases must be unique across the variants; a `_validation`
block enforces this at export time.

## Encoding

`encoding` tells the codegen how to translate the logical value to and from the
camera's wire representation. It has its own `kind`:

| `encoding.kind` | What it means                                            |
| --------------- | -------------------------------------------------------- |
| `raw`           | Wire value equals logical value.                         |
| `scale`         | Wire value = logical * `spec.scale`. Integer or float.   |
| `lookup`        | Explicit map from logical key (string) to wire value(s). |

Every encoding may carry an optional `prop_code: uint16`. Present means the
option is a top-level PTP device property (readable / writable via
`GetDevicePropValue` / `SetDevicePropValue`); absent means the option is only
used as a positional slot inside a render profile.

### `raw`

```cue
encoding: {kind: "raw", prop_code?: uint16}
```

Used for strings and for numeric options whose logical and wire representations
coincide.

### `scale`

```cue
encoding: {kind: "scale", prop_code?: uint16, spec: {scale: int}}
```

The wire value is `logical * scale`. Used when the camera uses fixed- point
storage for nominally fractional values (`highlight_tone`'s `scale: 10` means
`1.5` rides the wire as `15`).

### `lookup`

```cue
encoding: {
    kind: "lookup"
    prop_code?: uint16
    spec: values: {
        "<logical-key>": int | [int, ...int]   // single canonical or canonical+alternates
    }
}
```

- For `enum` kinds, the keys must be exactly the variant ids; `close({...})` in
  CUE enforces this.
- For `integer` lookup, keys must parse as integers; for `float` lookup, as
  floats. Codegen generates `Plus`/`Minus`/`Zero`-prefixed variants (`Plus4`,
  `Minus0_3`, `Zero`).
- Values are either a single wire int or a list. The first element is the
  canonical wire value (used when serializing); the rest are alternates accepted
  at deserialization (older firmwares, undocumented spellings).

Wire values must collectively fit in either `i16` or `u16`. Codegen picks `i16`
if any value is negative, `u16` otherwise, and bails if the range spans both
(e.g. `[-1, 40000]`).

## `codegen` Block

Optional knobs that shape what codegen emits:

```cue
codegen: {
    skip?:  true  // do not expose this option to any user-facing surface
    flaky?: true  // tolerate device-side set failures for this option
}
```

`skip` is used for options like `custom_setting` or `usb_mode` that are managed
by `fujicli` itself, not by the user. The option still has a generated type and
`SimulationSetting` impl; it just doesn't appear on `SimulationArgs` /
`RenderArgs`, or any future user-facing list.

`flaky` marks an option whose set can be rejected by the camera depending on
hardware state - e.g. `lens_modulation_optimizer`, which the camera refuses to
set with a PTP error when no lens is attached. In the generated `try_push`, a
flaky field's set is wrapped so that a device-side rejection is logged
(`log::warn!`) and skipped instead of aborting the whole simulation write; the
remaining fields still land. Codegen also emits a `cargo:warning` naming each
flaky option, so a field never becomes best-effort silently.

## What Gets Generated

For every option, codegen emits a self-contained module of impls. Roughly:

| Impl                                        | Always?            | Notes                                 |
| ------------------------------------------- | ------------------ | ------------------------------------- |
| `Debug, Clone, Copy/Eq, PartialEq`          | Yes                | `Copy` everywhere except strings.     |
| `Display`                                   | Yes                | Friendly form (variant name / value). |
| `FromStr`                                   | Yes                | Validates rules, accepts aliases.     |
| `Serialize / Deserialize`                   | Yes                | JSON-friendly.                        |
| `TryFrom<i32>` / `TryFrom<f32>`             | Numeric            | Range + step check.                   |
| `From<Self> for i32` / `From<Self> for f32` | Numeric            | Lossless reverse.                     |
| `PtpSerialize, PtpDeserialize`              | Yes                | PTP wire codec.                       |
| `SimulationSetting` (carries `prop_code()`) | If `prop_code` set | Used by `try_pull` / `try_push`.      |
| `ConversionProfileField`                    | Yes                | 32-bit lifted codec for render slots. |

The unifying constraint: numeric scaled options always use `i16`/`u16` on the
wire (Fuji's convention) and `i32` logical / `i32` profile-slot. The `signed`
determination is made from the rules' min or the lookup values' range.
