# Generations

A _generation_ groups cameras that share underlying behaviour. For example, the
X-Trans IV bodies use the same simulation setting list, and so on. A generation
block holds (a) presentational metadata and (b) "capability templates" that
cameras refer to wholesale.

Generations are defined in [`fml/generation.cue`](../../fml/generation.cue).

```cue
generations: x_trans_v: #Generation & {
    spec: {
        name: "X-Trans V"

        _simulation: {
            settings: [...]  // ordered list of #Setting
            rules:    [...]  // shared rules
        }

        // Future: _render, _backup, etc.
    }
}
```

The `_simulation` block is a CUE convention (leading underscore = template, not
a field the schema validator cares about per se). It exists so cameras can do
`settings: _generation._simulation.settings` in their own feature block and
inherit the whole list.

## Inheritance

Generations don't impose anything; cameras opt in by referencing templates. The
`_generation` field on `#Camera.spec` is the bridge:

```cue
// fml/camera.cue
x_t5: #Camera & {
    spec: {
        name:        "FUJIFILM X-T5"
        generation:  "x_trans_v"
        _generation: _  // brings in generations[generation].spec
        // ...
        features: simulation: {
            slots:    7
            settings: _generation._simulation.settings
            rules:    _generation._simulation.rules
        }
    }
}
```

`_generation: _` is what activates the lookup
`_generation:
generations["\(generation)"].spec`. Without it, the camera block
can't see the generation's templates.

## Extending and Overriding

CUE unification means you can layer extras on top of inherited lists with
`list.Concat`:

```cue
settings: list.Concat([
    _generation._simulation.settings,
    [{ref: "new_thing"}],
])

rules: list.Concat([
    _generation._simulation.rules,
    [
        {
            message: "..."
            when: ...
        },
    ],
])
```

This is how the X-Trans V generation builds on X-Trans IV by adding
`smooth_skin_effect`. The codegen treats the merged list as the canonical one;
nothing knows or cares which entries came from where.

## What This Enables

A single change to a generation propagates to every camera that references it.
Concretely:

- Adding a setting to `x_trans_v._simulation.settings` makes every X-Trans V
  body's simulation struct gain that field on the next build.
- Adding a rule to `x_trans_v._simulation.rules` propagates validation and (if
  it matches the presence-anchor pattern) read gating across all those cameras.

## Current State

The existing generations carry only `_simulation` templates because that's the
feature that's been reversed across the family. Backup and rendering are still
per-camera. As more cameras are reversed, move common behaviour up.

A typical migration:

1. Reverse one camera (see [reversing](../contributors/reversing.md)).
2. Land per-camera spec (see
   [adding cameras](../contributors/adding-cameras.md)).
3. Reverse a second camera in the same generation.
4. If the two specs agree, lift the common block into `_render` on the
   generation and reduce both cameras to `render: _generation._render`.
