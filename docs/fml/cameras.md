# Cameras

A _camera_ block declares a model: how it talks USB-side, what generation it
belongs to, and what features `fujicli` should expose for it. Defined in
[`fml/camera.cue`](../../fml/camera.cue).

```cue
cameras: <id>: #Camera & {
    spec: {
        name:        string          // marketing name
        generation:  #RefGeneration  // id of a generation
        _generation: _               // expands to generations[generation].spec
        usb:         #USB
        features?:   #Features
    }
}
```

`<id>` is the camera's stable Rust ident; every generated struct and constant is
derived from it (e.g. `x_t5` -> `XT5`, `XT5Simulation`, `XT5RenderProfile`,
`C_X_T5` in the support registry).

## USB

```cue
usb: {
    vendor_id:  uint16 | *0x04cb   // Fujifilm by default
    product_id: uint16             // required
    chunk_size: uint   | *1048576  // PTP bulk transfer chunk, default 1 MiB
}
```

Set `chunk_size` higher than the default if the camera tolerates it (the X-T5
uses ~16 MiB and renders noticeably faster as a result). Reverse-engineer by
experiment: too-large chunks cause timeouts or errors.

## Features

```cue
features?: {
    backup?:     true
    simulation?: #Simulation
    render?:     #Render
}
```

A missing key means "not supported".

### Backup

`backup` is the only feature without an inline schema. It's a boolean toggle
because the underlying PTP exchange is uniform across the supported models.

### Simulation

```cue
simulation: {
    slots: uint             // C1 .. C<slots>
    settings: [...{
        id:  string | *ref  // defaults to ref
        ref: #RefOption     // must name a declared option
    }]
    transformations?: [...]
    rules?:           [...]
}
```

`settings` is the **ordered** list of options this camera's simulation profile
is composed of. Order matters for PTP read/write; the codegen runs them in
declaration order, perturbed only by gating dependencies inferred from rules.
See [rules](rules.md) for how validation, repair, and read-gating are derived
from the rule list, and [grammar](grammar.md) for the predicate language those
rules use.

If your camera shares its generation's profile verbatim, set
`settings: _generation._simulation.settings` and inherit the rules the same way.

### Render

```cue
render: {
    profile_code: uint32  // identifies the wire format
    fields: [...#Field]   // ordered list of slots
    transformations?: [...]
    rules?:           [...]
}

#Field:
    | {id: string, ref: #RefOption, skip_read?: true, skip_write?: true}
    | {id: string,                  skip_read?: true, skip_write?: true}
```

Fields fall into two kinds:

- **`ref` fields** inherit type and encoding from a declared option
  (`{ref: "image_size"}`). The `id` defaults to the ref. The field participates
  in CLI args and bidirectional value mapping.
- **Inline fields** carry only an `id`, no `ref`. They are bare `i32` slots used
  for headers, trailers, and undocumented padding values (`{id: "head_0"}` /
  `{id: "tail_0", skip_read: true}`). They never appear on the CLI.

`skip_read` / `skip_write` make a slot one-way.

`profile_code` is part of the wire format. The camera enforces it on read.
Reverse it before guessing (see [reversing](../contributors/reversing.md)).

## CUE-Side Validation

Each feature block carries an `_validation` field that CUE evaluates at export
time:

```cue
_validation: {
    ids: list.UniqueItems & [for s in settings {s.id}]
}
```

Setting / field `id`s must be unique. If you accidentally introduce a duplicate,
`cue export` fails with a precise location.

## Available Predicates

Inside a camera's simulation or render block, the `#Grammar` you write rules and
transformations in is _typed by the local setting list_. That means:

- A predicate `{ref: "image_size", lt: 5}` is rejected at CUE export because
  `image_size` is an enum.
- A predicate `{ref: "monochromatic_color_temperature", gte: 0}` only works
  inside a camera/generation whose settings include that field.
- `{ref: "monochromatic_color_tint", present: true}` works against any `id` in
  scope.

This pre-flight check catches the most common bugs before codegen ever runs. See
[grammar](grammar.md) for the full leaf vocabulary.
