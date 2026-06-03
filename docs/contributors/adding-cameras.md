# Adding a Camera

A camera is a CUE block in [fml/camera.cue](../../fml/camera.cue). At a minimum
it has a USB ID and a generation; that alone is enough to make `device list` and
`device info` work. Backups, simulations, and rendering each unlock a feature
block.

For the language reference see [FML / cameras](../fml/cameras.md). This page is
the recipe.

## Before You Start

You will need:

- A real camera of that model. The schema is permissive about USB IDs but
  feature blocks are claims about behaviour you have to verify.
- A `?` entry in [the support table](../users/support.md) - that means the USB
  ID is already wired up. If your model isn't there, you'll add the basic block
  first.
- The [reversing](reversing.md) workflow to probe what the camera actually
  supports before declaring it.

## 1. Minimal Camera Block

Open `fml/camera.cue` and add (or confirm) an entry:

```cue
x_t40: #Camera & {
    spec: {
        name:       "FUJIFILM X-T40"  // exact marketing name
        generation: "x_trans_v"       // see fml/generation.cue
        usb: product_id: 0x1234       // hex, from `lsusb` or vendor docs
    }
}
```

The key on the left (`x_t40` above) is the camera's stable id. It must be a
valid Rust identifier; if your name starts with a digit, prefix it with an
underscore - codegen will turn `X100VI` into `X100Vi` and similar.

`usb.vendor_id` defaults to Fujifilm (`0x04cb`); set it only if you're testing a
re-badged unit. `usb.chunk_size` defaults to 1 MiB; raise it if the camera
tolerates larger PTP bulk transfers (the X-T5 uses ~16 MiB).

Run `cargo build` once. The build will:

1. `cue export ./fml --out json` - CUE validates the entire spec.
2. `codegen::generate` - emits the per-camera struct and registry entry into
   `src/lib/generated/`.

If anything fails, the CUE error will pinpoint the bad field.

## 2. Enable Backup

Probe with
[`fujicli device reverse backup export`/`import`](reversing.md#backup)
**first**; the non-reverse `fujicli backup` commands won't run until the feature
is declared. Once you've round-tripped a backup that way, set:

```cue
features: backup: true
```

That's the entire feature block. Backup is the same PTP dance for every Fuji
camera we've seen.

## 3. Enable Simulations

Probe with [`fujicli device reverse simulation`](reversing.md#simulation) to
confirm which PTP property codes the camera responds to. Then declare the
feature block.

Most cameras can share the generation's simulation settings, in which case the
feature block is short:

```cue
features: {
    backup: true

    simulation: {
        slots:    7  // C1..C7 on most bodies
        settings: _generation._simulation.settings
        rules:    _generation._simulation.rules
    }
}
```

`_generation._simulation.settings` is the list of PTP properties that generation
reads/writes, ordered as the camera expects. If your camera has a setting the
generation doesn't, declare it inline:

```cue
simulation: {
    slots: 4
    settings: [
        // ... generation defaults ...
        _generation._simulation.settings,
        {ref: "exotic_new_thing"},
    ]
    rules: [
        ...generation._simulation.rules,
        {
            message: "Exotic thing requires X-Y mode."
            when: all: [
                {ref: "exotic_new_thing", present: true},
                {not: {ref: "mode", equals: "x_y"}},
            ]
        },
    ]
}
```

If the option `exotic_new_thing` doesn't exist yet, add it to
[fml/option.cue](../../fml/option.cue) first; see
[fml / options](../fml/options.md).

Once the feature block is in place, verify the runtime side:

```sh
cargo run -- simulation get c1
cargo run -- simulation set c1 --film-simulation provia
```

Both should round-trip without errors.

### 4. Pin Allowed Values

When you declare a camera, **add rules that explicitly bound every option's
allowed values and ranges** for that camera, even when the option already has
permissive global rules.

The reason is forward-compatibility: when Fujifilm introduces a new film
simulation or expands a range in a future firmware, we want to add the new value
to the option globally without retroactively claiming every older camera
supports it.

Write the same shape per option even if the rule overlaps the global one. Yes,
this is verbose; the point is that it locks the camera spec to today's reality
so option-level changes can't silently extend it.

## 4. Enable Rendering

Rendering is the most invasive feature: it requires a per-camera `render` block
listing the _exact_ sequence of fields in the camera's conversion-profile wire
format, plus any `transformations` that flatten user-facing values into
wire-level ones.

```cue
features: {
    // ... backup, simulation ...

    render: {
        profile_code: 0xff179502    // unique per camera; see reversing
        fields: [
            {id: "head_0"},                          // inline pad
            {ref: "file_type"},
            {ref: "image_size"},
            // ... 25-30 more fields, in wire order ...
            {id: "tail_0", skip_read: true},
        ]
        transformations: [
            {
                when: {ref: "dynamic_range", equals: "hdr800_plus"}
                apply: [
                    {ref: "dynamic_range",          value: "hdr800"},
                    {ref: "dynamic_range_priority", value: "plus"},
                ]
            },
            // ...
        ]
        rules: [
            // Same shape as simulation rules. Pin allowed values per camera
            // here too - same reasoning as for simulations.
            //
            // Render rules can additionally use `scope: "original"` on a
            // leaf to refer to the camera-reported value of an option
            // *before* the user's partial was merged.
        ]
    }
}
```

Field semantics:

- `ref` - name of an option declared in `fml/option.cue`. Inherits its type and
  encoding.
- `id` (without `ref`) - an inline `i32` slot. Used for headers, trailers, and
  undocumented padding. Set `skip_read` or `skip_write` if the slot is one-way.
- Order is significant. The wire format is
  `[i16 n_props][hex profile_code][padding][i32 * n_props]`, in declaration
  order.

See [internals / codegen](../internals/codegen.md#renders) for what the emitter
does with this, and [reversing](reversing.md) for how to discover
`profile_code`, field order, and padding for a new model.

## 5. Update the Support Table

When you add or upgrade a camera, edit
[docs/users/support.md](../users/support.md) to reflect what works. Use `Y` only
for features you've personally verified.
