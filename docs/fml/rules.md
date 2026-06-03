# Rules and Transformations

Rules and transformations are how cameras and generations encode domain
knowledge: which combinations of settings are valid, which user-facing values
translate to multi-field wire encodings, and which fields are only meaningful in
certain modes. They are the heart of the schema; understanding their semantics
is how you write a correct spec.

## Rules

```cue
rules: [...{
    severity: "error" | "warning" | "info"   // default "error"
    message:  string
    when:     #Predicate                     // see grammar.md
}]
```

A rule fires when `when` evaluates to true against the current record. What
"fires" means depends on severity:

| Severity  | Effect                                                                                           |
| --------- | ------------------------------------------------------------------------------------------------ |
| `error`   | `solve` attempts to **repair** the record. If repair fails, the operation aborts with `message`. |
| `warning` | `log::warn!(message)` at runtime. Never blocks.                                                  |
| `info`    | `log::info!(message)` at runtime. Never blocks.                                                  |

Errors also drive
[presence-DAG extraction](../internals/analyses.md#the-presence-dag), which is
how the read path knows when to skip optional fields.

### Conventional Rule Shape

The pattern most rules follow:

```cue
{
    message: "X is only meaningful in Y mode."
    when: all: [
        {ref: "x", present: true},            // anchor, gates the rest
        {not: {ref: "y", equals: "y_mode"}},  // condition under which X is invalid
    ]
}
```

Read as: "X being set is inconsistent with Y != y_mode". The runtime gets two
things from this one rule:

1. **Validation.** If a user writes a partial with `X = some_value` and
   `Y = something_else`, repair will try to clear X (or change Y to `y_mode`)
   before bailing with the message.
2. **Read gating.** When reading the camera state, X is read only if the gate
   `Y == "y_mode"` holds.

That dual effect is intentional. Authors who write rules in this shape get
read-time inference "for free". See
[adding cameras](../contributors/adding-cameras.md) for examples.

### Rule Ordering Matters

`solve` attempts repair in declaration order. If repairing rule N breaks rule M
and vice versa, the engine bails - `re_fires_other_ok` catches the cycle but
does not search alternatives. Order rules from **most specific to most general**
so the heuristic finds a fix on the first pass.

## Transformations

```cue
transformations: [...{
    when?: #Predicate        // optional gate; default = unconditional
    apply: [...#Assignment]  // assignments to run
}]
```

Transformations rewrite the field set. They run in two contexts:

1. **`apply_transformations()`** is called inside `try_update_from` _before_
   `solve`. The rewrites land on the candidate; `solve` validates the rewritten
   record.
2. **On render deserialization**, _invertible_ transformations are inverted in
   reverse declaration order to lift wire-level fields back to user-facing ones.

A transformation is **invertible** iff:

- Its `when` is a single `equals` leaf (no compound triggers); **and**
- No other transformation in the list shares the same `apply` field / value
  pattern.

The first constraint makes the inverse unambiguous to detect _on read_; the
second makes it unambiguous to _attribute_. If those conditions don't hold, the
transformation is one-way. Codegen emits a `cargo:warning` listing it.

### Example: Value Flattening

The X-T5's wire format treats `hdr800_plus` as a flag combination:

```cue
transformations: [
    {
        when: {ref: "dynamic_range", equals: "hdr800_plus"}
        apply: [
            {ref: "dynamic_range",          value: "hdr800"},
            {ref: "dynamic_range_priority", value: "plus"},
        ]
    },
]
```

- **Write path.** When the user sets `dynamic_range = hdr800_plus`,
  `apply_transformations` rewrites that into `(hdr800, plus)` before validation.
  The rule that says
  `dynamic_range_priority must be off
  when dynamic_range is set` sees the
  rewritten pair and can repair if needed.
- **Read path.** When deserializing a render profile, the inverse pass sees
  `(hdr800, plus)` and rewrites back to `hdr800_plus`. The user never deals with
  the flat form.

### Example: Unconditional Padding

```cue
transformations: [
    {
        apply: [
            {ref: "head_0", value: 0},
            {ref: "tail_0", value: 0},
        ]
    },
]
```

No `when`; runs every time. Used to enforce default values for inline render
slots without bothering the user.

### How Aliasing Interacts with Rules

When a rule is normalized:

1. The rule's predicate is reduced to DNF.
2. For each transformation with a non-empty `apply`, attempt to substitute its
   `trigger` into each conjunction. If a conjunction contains all the trigger's
   leaves (multiset-subset), remove them and append the transformation's `apply`
   leaves.
3. The result is the "effective rule" the validation, repair, and gating
   analyses operate on.

This is how a rule written against `hdr800_plus` is automatically phrased in
terms of `(hdr800, plus)` for analysis, even though the user types
`hdr800_plus`.

Substitution is **first-match-wins** per conjunction. Two transformations with
the same trigger silently lose the second. Chains _do_ work - three aliases A ->
B, B -> C will substitute through in declaration order.

## Putting It Together - A Complete Simulation Block

```cue
simulation: {
    slots:    7
    settings: _generation._simulation.settings

    transformations: [
        // Flatten the user-facing combination into wire-level fields.
        {
            when: {ref: "dynamic_range", equals: "hdr800_plus"}
            apply: [
                {ref: "dynamic_range",          value: "hdr800"},
                {ref: "dynamic_range_priority", value: "plus"},
            ]
        },
    ]

    rules: [
        {
            message: "Monochromatic color settings only apply to black and white simulations."
            when: all: [
                {any: [
                    {ref: "monochromatic_color_temperature", present: true},
                    {ref: "monochromatic_color_tint",        present: true},
                ]}
                {not: {
                    ref: "film_simulation"
                    in: ["monochrome", "monochrome_ye", "monochrome_r", "monochrome_g",
                         "acros", "acros_ye", "acros_r", "acros_g"]
                }}
            ]
        },
        {
            message: "White balance temperature is only meaningful in Temperature mode."
            when: all: [
                {ref: "white_balance_temperature", present: true},
                {not: {ref: "white_balance", equals: "temperature"}},
            ]
        },
        {
            message: "Dynamic Range can only be set when Dynamic Range Priority is disabled."
            when: all: [
                {ref: "dynamic_range", present: true},
                {not: {ref: "dynamic_range_priority", equals: "off"}},
            ]
        },
    ]
}
```

What this gives you:

- `monochromatic_color_*` are optional fields, read only when `film_simulation`
  is one of the listed monochrome variants.
- `white_balance_temperature` is optional, read only when
  `white_balance == "temperature"`.
- `dynamic_range` is read only when `dynamic_range_priority == "off"`.
- Read order respects all of the above (gating refs must be read before their
  anchors).
- `hdr800_plus` works on input even though the camera doesn't have that
  primitive.
- `solve` will try to repair contradictory partials by clearing whichever side
  the user didn't pin.

That's roughly 25 lines of CUE producing several hundred lines of emitted Rust
that implements all four things consistently.

## Cross-State Rules

Render rules can refer to the pre-merge state via `scope: "original"` - the
camera's reported render profile before the user's partial was merged.

```cue
{
    message: "Dynamic Range cannot exceed the value the image was shot with."
    when: any: [
        {all: [
            {ref: "dynamic_range", scope: "original", equals: "hdr200"},
            {not: {ref: "dynamic_range", in: ["hdr100", "hdr200"]}},
        ]},
        {all: [
            {ref: "dynamic_range", scope: "original", equals: "hdr400"},
            {not: {ref: "dynamic_range", in: ["hdr100", "hdr200", "hdr400"]}},
        ]},
        // ... and so on
    ]
}
```

If the only fix would be to change the original, `solve` bails with the rule's
message - the camera shot it that way, we can't undo it. See
[analyses / scope](../internals/analyses.md#scope) for how each pass handles
original-scope leaves.

## When to Use What

| Need                                                        | Use                                                                                         |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| "These two values are incompatible."                        | A rule with `severity: error`.                                                              |
| "This field doesn't make sense when..."                     | A rule with the standard `Present(X) && ~cond` shape. The presence-DAG handles read-gating. |
| "Allow `compound_value`; flatten to (a, b)."                | A transformation with `when: equals` and a multi-`apply`.                                   |
| "Set a default for an inline render slot."                  | An unconditional transformation.                                                            |
| "Suggest something to the user."                            | A rule with `severity: warning`.                                                            |
| "Log when an unusual combination appears."                  | A rule with `severity: info`.                                                               |
| "Constrain a render based on what the image was shot with." | A render rule that references `scope: "original"`.                                          |
