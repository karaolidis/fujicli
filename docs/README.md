# fujicli Documentation

## For Users

- [Installation](users/installation.md) - per-platform setup.
- [Usage](users/usage.md) - CLI walkthrough with examples.
- [Camera Support](users/support.md) - what works on which model.

## For Contributors

- [Contributing](contributors/README.md) - workflow, branches, formatting.
- [Adding a Camera](contributors/adding-cameras.md) - the most common
  contribution.
- [Reversing](contributors/reversing.md) - capturing PTP traffic from Fujifilm's
  official tools.

## FML Reference

- [Overview](fml/README.md) - what FML is, how the pieces fit together.
- [Options](fml/options.md) - typed scalars (`integer`, `float`, `string`,
  `enum`) with encoding rules.
- [Cameras](fml/cameras.md) - per-model spec: USB IDs, feature blocks.
- [Generations](fml/generations.md) - shared capability presets.
- [Grammar](fml/grammar.md) - predicates and assignments.
- [Rules and Transformations](fml/rules.md) - validation, repair, aliasing
  semantics.

## Internals

- [Architecture](internals/README.md) - the pipeline at a glance.
- [Codegen](internals/codegen.md) - what each emitter produces.
- [Analyses](internals/analyses.md) - DNF, alias substitution, the presence DAG,
  repair, and inverse transformations.
- [Runtime](internals/runtime.md) - how the generated modules are consumed.
