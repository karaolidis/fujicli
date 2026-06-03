# Grammar

The _shared vocabulary_ that rules and transformations are written in. Defined
in [`fml/grammar.cue`](../../fml/grammar.cue) as `#GrammarBase`, then
specialized inside each camera's simulation or render block with the local
setting list as its scope.

## Predicates

A predicate is either a leaf or a logical combinator.

```cue
#Predicate: #Logic | #Leaf

#Logic:
    | {all: [...#Predicate]}  // conjunction
    | {any: [...#Predicate]}  // disjunction
    | {not:    #Predicate}    // negation
```

Empty `all` = tautology (always true). Empty `any` = contradiction (always
false). `bool` literals (`true` / `false`) are also allowed as predicates.

### Leaves

Every leaf names a `ref` (a setting id in scope) plus exactly one operator key:

| Operator key       | Applies to                | Meaning                                 |
| ------------------ | ------------------------- | --------------------------------------- |
| `equals: <value>`  | integer/float/string/enum | `ref == value`                          |
| `in: [<value>...]` | integer/float/string/enum | `ref in list`                           |
| `min, max`         | integer/float             | `min <= ref <= max` (both required)     |
| `lt: <value>`      | integer/float             | `ref < value`                           |
| `lte: <value>`     | integer/float             | `ref <= value`                          |
| `gt: <value>`      | integer/float             | `ref > value`                           |
| `gte: <value>`     | integer/float             | `ref >= value`                          |
| `present: bool`    | any                       | `ref` is `Some` (true) / `None` (false) |

Some examples:

```cue
{ref: "film_simulation", equals: "provia"}
{ref: "noise_reduction", in: [-4, -2, 0]}
{ref: "highlight_tone",  gte: 0.0}
{ref: "image_quality",   present: false}
{any: [
    {ref: "monochromatic_color_temperature", present: true},
    {ref: "monochromatic_color_tint",        present: true},
]}
```

### Scope

Every leaf also carries `scope: "current" | "original"`, defaulting to
`"current"`. `"original"` reads from the pre-merge snapshot and is only
available in render-block _rules_ (the camera-reported profile before the user's
partial). Transformation `when` predicates are restricted to current scope.
Assignments never carry scope; you cannot write to the original. See
[rules / cross-state rules](rules.md#cross-state-rules) for an
example and [analyses / scope](../internals/analyses.md#scope) for the per-pass
behaviour.

### Type Discipline

`#GrammarBase._ids` carries four typed-ref sets keyed on the option kind:

- `_ids.i` - integer-kind settings
- `_ids.f` - float-kind settings
- `_ids.s` - string-kind settings
- `_ids.e` - enum-kind settings
- `_ids.all` - every setting in scope

The leaf type definitions constrain `ref` to the appropriate set, which is why
CUE rejects `{ref: "image_size", lt: 5}` at export (`image_size` is an enum, not
in `_ids.i`).

`#GrammarBase` also carries a `_scoped: bool` toggle. Render blocks set it true
to admit `scope: "original"`; simulation blocks leave it false, which restricts
`scope` to `"current"`.

For enum `equals` / `in`, CUE also restricts the value side:

```cue
#PredicateEnumEquals: {
    ref: or(_ids.e)
    equals: or([for v in options[ref].spec.rules.variants {v.id}])
}
```

Typing a non-existent variant fails at export.

## Assignments

Used in `transformations.apply`. Same shape as leaves but with one of `value` or
`present: false`:

```cue
#AssignmentInteger: {ref: or(_ids.i), value: int}
#AssignmentFloat:   {ref: or(_ids.f), value: float}
#AssignmentString:  {ref: or(_ids.s), value: string}
#AssignmentEnum: {
    ref: or(_ids.e)
    value: or([for v in options[ref].spec.rules.variants {v.id}])
}
#AssignmentClear:   {ref: or(_ids.all), present: false}
```

Setting a value means `field = Some(value)`. Clearing means `field =
None`.
There is no "merge" assignment; express that as multiple `apply` entries.

## Compilation

Each leaf becomes a Rust boolean expression over `self.<field>`, or
`original.<field>` when `scope: "original"`. Sketch:

```
{ref: "x", equals: 5}     (integer)        -> self.x.is_some_and(|v| i32::from(v) == 5i32)
{ref: "x", scope: "original", equals: 5}   -> original.x.is_some_and(|v| i32::from(v) == 5i32)
{ref: "x", in: ["a", "b"]} (enum)          -> self.x.is_some_and(|v| matches!(v, Path::A | Path::B))
{ref: "x", min: -1.0, max: 1.0} (f32)      -> self.x.is_some_and(|v| (-1.0..=1.0).contains(&f32::from(v)))
{ref: "x", present: true}                  -> self.x.is_some()
{all: [P, Q]}                              -> (P) && (Q)
{any: [P, Q]}                              -> (P) || (Q)
{not: P}                                   -> !(P)
```

Float equality uses `(v - x).abs() < f32::EPSILON`, not raw `==`. Strings use
`as_deref().is_some_and(|v| v == "...")`. Enums use typed `matches!` arms.

See [internals / analyses](../internals/analyses.md) for what happens _before_
compilation; predicates are normalized to DNF, aliased through transformations,
and used to derive read-gating dependencies.
