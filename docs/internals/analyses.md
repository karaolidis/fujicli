# Analyses

The clever bits. Each pass turns one form of the AST into another that's better
suited to the next step. The whole pipeline:

```
Predicate                          (ast/grammar.rs)
  | Dnf::from
  v
Dnf = Vec<Conjunction<Leaf>>       (ast/dnf.rs)
  | alias substitution
  v
NormalizedRule { when: Dnf, ... }  (schema/alias.rs)
  |
  +-- PresenceDag::try_from_rules -- used to derive read order + gates  (schema/presence.rs)
  +-- generate_solve              -- used to emit `solve()` + repair    (schema/repair.rs)
  +-- generate_emit_warnings_...  -- used to emit log calls             (schema/grammar.rs)
  v
TokenStream                        (schema/grammar.rs + emitters)
```

Plus a separate pass on transformations for invertibility detection
(`schema/inverse.rs`).

## Scope

Every `Leaf` carries `scope: Scope` (`Current` or `Original`, defaulting to
`Current`). The codegen helpers in
[`schema/grammar.rs`](../../crates/codegen/src/schema/grammar.rs) take a
`Scopes { current: &TokenStream, original: Option<&TokenStream> }` and route
each leaf to the right accessor via `Scopes::pick(scope)`. An `Original` leaf in
a context with no original accessor fires `unreachable!()` - CUE typing should
keep that path unreachable.

Bindings differ by call site:

| Site                                            | `current` | `original` |
| ----------------------------------------------- | --------- | ---------- |
| `solve` seeds + repair walks                    | `self`    | `original` |
| `re_fires_other_ok`                             | `state`   | `original` |
| `emit_warnings_and_infos`                       | `self`    | `original` |
| `apply_transformations`                         | `self`    | none       |
| Simulation `try_pull`                           | `staged`  | none       |
| Render deserialization (`generate_convert_one`) | `profile` | none       |

The render path snapshots `original = self.clone()` before merging the user
partial in `try_update_from`; simulation has no original because simulation
settings carry no immutable shot-time state. Read deserialization runs over wire
bytes alone; there is no "original" yet (it's what the read is producing). The
per-pass sections below note where scope changes behaviour.

## DNF Normalization

[`ast/dnf.rs`](../../crates/codegen/src/ast/dnf.rs) turns any `Predicate` into a
disjunction of conjunctions of _literals_. A literal (`Leaf`) carries its
polarity explicitly - `NotEquals`, `NotIn`, etc. - so the structural negation
`Not(...)` is gone from the normal form.

The shape:

```rust
pub struct Conjunction(pub Vec<Leaf>);  // && of literals
pub struct Dnf(pub Vec<Conjunction>);   // || of conjunctions
```

Two degenerate forms have well-defined meanings:

- **Tautology** - any conjunction is empty. `Dnf::is_tautology`.
- **Contradiction** - the disjunction is empty. `Dnf::is_contradiction`.

Normalization rules:

| Input              | Output                                                               |
| ------------------ | -------------------------------------------------------------------- |
| `Bool(true)`       | `Dnf([Conjunction([])])` - single empty disjunct (tautology)         |
| `Bool(false)`      | `Dnf([])` - empty disjunction (contradiction)                        |
| Leaf `L`           | `Dnf([Conjunction([Leaf(L)])])`                                      |
| `All([P, Q])`      | Cross product: each `P` disjunct concatenated with each `Q` disjunct |
| `Any([P, Q])`      | Disjuncts of `P` and `Q` concatenated                                |
| `Not(All([P, Q]))` | `Any([Not(P), Not(Q)])` then normalized (De Morgan)                  |
| `Not(Any([P, Q]))` | `All([Not(P), Not(Q)])` then normalized                              |
| `Not(Not(P))`      | `P` normalized (double negation)                                     |
| `Not(Leaf)`        | Leaf with polarity flipped (`Equals` <-> `NotEquals`, etc.)          |

The output is _not_ minimal. That's intentional: the downstream analyses (alias
substitution, presence DAG, repair) all benefit from seeing every disjunct
explicitly, and minimization would muddle that.

Conjunction equality is **multiset** equality
([`util::multiset`](../../crates/codegen/src/util/multiset.rs)) so `A && B && A`
!= `A && B`, but `A && B` == `B && A`. Hashing matches.

Scope is part of `Leaf::PartialEq` (and `Hash`), so a `Current` leaf and an
`Original` leaf compare unequal even if their other fields match.

## Alias Substitution

[`schema/alias.rs`](../../crates/codegen/src/schema/alias.rs) normalizes a
`Transformation` into a `NormalizedTransformation`:

```rust
struct NormalizedTransformation {
    trigger:   Dnf,          // when as a DNF
    expansion: Conjunction,  // apply as a conjunction of leaves
}
```

A clear-assignment (`{ref, present: false}`) becomes `Present(ref, false)`; a
set-assignment becomes `Equals(ref, value)`.

To apply aliases to a rule:

```rust
let mut dnf = Dnf::from(rule.when);
for alias in aliases {
    dnf = dnf.transform(alias);
}
```

`transform` walks each conjunction. For each disjunct in the alias's `trigger`,
check whether the conjunction contains all of the disjunct's literals as a
multiset-subset. If so, remove them and append the alias's `expansion`. **First
match wins per conjunction per alias**.

Important properties:

- **Order-insensitive trigger matching.** `{all: [A=1, B=2]}` matches a
  conjunction that contains those literals in any order.
- **Superset matching.** A conjunction `{A=1, B=2, C=3}` matches the trigger
  `{all: [A=1, B=2]}` - `C=3` survives, `(A, B)` is rewritten to the expansion.
- **Chained aliases work in declaration order.** Alias 1 rewrites the
  conjunction; alias 2 sees the rewritten form.
- **Duplicate triggers silently lose.** Two aliases with identical triggers:
  only the first ever fires.
- **`Any` triggers fan out.** Each disjunct of the trigger is checked
  independently against the conjunction.

The resulting `NormalizedRule` carries the normalized DNF, the original
severity, and the original message.

Aliases come from transformations whose `apply` always produces `Current`-scope
leaves. Trigger matching uses multiset-subset over `Leaf`, which includes scope
in equality, so original-scope literals in a rule conjunction are inert under
substitution.

## The Presence DAG

[`schema/presence.rs`](../../crates/codegen/src/schema/presence.rs) is the
highest-leverage analysis. From the error rules alone, it derives:

- **Edges**: dependencies between settings, used to topologically order reads.
- **Conditions**: per-setting predicates used to gate reads on the fly.

For each error rule, for each conjunction in its DNF:

1. Drop original-scope leaves. They describe a relationship between the
   pre-merge snapshot and the candidate; the read path has no "original" yet, so
   they cannot anchor or gate a read.
2. Partition the remaining literals into `Present(X, true)` anchors,
   `Present(X, false)` anchors, and "other" literals.
3. Pick a polarity:
   - If there are any `true` anchors -> polarity is `true`; targets are the
     true-anchors; false-anchors fold into the gating clauses.
   - Else if there are only `false` anchors -> polarity is `false`; targets are
     the false-anchors; gating clauses are as-is.
   - Else (only "other" literals) - this rule has no presence anchor, so it
     doesn't contribute to the DAG. It still validates.
4. Collect `gating_refs` - every ref mentioned in the gating clauses.
5. Reject self-gating: if any target ref appears in `gating_refs`, bail.
   Deciding whether to read X would require knowing X's value.
6. For each (gating_ref, target) pair, add an edge `gating_ref ->
   target`
   (target must be read after gating ref).
7. Compute the "gate" as a `Dnf`:
   - Polarity true -> gate is the negation of each gating literal, joined as
     separate disjuncts (`!cond -> skip X`).
   - Polarity false -> gate is the original gating literals as a single
     conjunction.
8. Append the gate to `contributions[target]`.

After iterating, each target's contributions are combined with `All` (every
contributing rule's gate must hold for the read to fire). The resulting
predicate is what `try_pull` wraps each read with.

The intuition: the rule

```
Present(X) && cond(Y)  // X being set is inconsistent with cond(Y)
```

means "X is only meaningful when !cond(Y)". The DAG transformation extracts
exactly that read-time predicate from the validation rule, and infers the
read-order edge `Y -> X` so the gate has the data it needs when it fires.

### Topological Sort

The collected nodes (settings in declaration order) and edges feed into
[`util::dag::Dag`](../../crates/codegen/src/util/dag.rs). Kahn's algorithm with
a min-heap keyed on **declaration index** yields the order closest to
declaration order among all valid topological orders. Where the spec doesn't
force an order, the read order matches what the FML author typed - which makes
the generated code readable and stable.

Cycles surface as `ordering cycle detected among settings: [...]`. Most commonly
caused by two rules with crossed gating (`A gates B` and `B gates A`).

## Repair

[`schema/repair.rs`](../../crates/codegen/src/schema/repair.rs) emits the
`solve(&mut self, pin: &HashSet<&'static str>)` function. The render path gets
an extra `original: &Self` parameter; original-scope leaves in the rule DNFs
read from that snapshot. Simulation `solve` keeps the original-free signature.

The emitted code does:

```rust
let mut ok = [false; N];
for i, rule in error_rules:
    ok[i] = !rule.when

for i, rule in error_rules:
    if !ok[i]:
        if !self.try_repair_rule_i(pin, &ok):
            bail!(rule.message)
        ok[i] = true
```

Each `try_repair_rule_i` is generated from the rule's DNF. It walks the DNF: for
every disjunct (a conjunction whose truth implies the rule firing), attempt to
break the disjunct by flipping one of its literals. The flip is only attempted
if the target field is not `pin`'d - i.e. the user did not explicitly set it.

Original-scope leaves are non-flippable; the walk skips them. If a disjunct has
no flippable literal left, repair falls through and the outer loop bails.

For each current-scope leaf, the flip is:

| Literal                                                                 | Flip                                          |
| ----------------------------------------------------------------------- | --------------------------------------------- |
| `Equals(X, v)` / `In(X, ...)` / `Between(X, ...)` / ordered comparisons | `self.X = None`                               |
| `Present(X, true)`                                                      | `self.X = None`                               |
| `Present(X, false)`                                                     | No flip (would require synthesising a value). |
| `NotEquals(X, v)`                                                       | `self.X = Some(v)` (the offending value)      |
| `NotIn(X, [v1, ...])`                                                   | `self.X = Some(v1)` (first witness)           |
| `NotBetween(X, [lo, hi])`                                               | `self.X = Some(lo)`                           |
| Negated ordered comparisons                                             | No flip (no witness).                         |

After every flip:

1. **`re_fires_other_ok`** - re-evaluate every other rule. If a rule that was OK
   now fails, the flip is rejected and reverted.
2. If accepted, the walk breaks out and the rule is considered repaired.

On total failure, the snapshot taken at the start of the walk is restored and
the function returns false. The outer loop then bails with the rule's message.

### Properties and Limitations

- **Unit-flip only.** No multi-field search. If repairing rule N requires
  changing both X _and_ Y, the engine can't find that. Workaround: write rules
  whose repair is achievable with a single flip, and let `apply_transformations`
  do the multi-field preparation.
- **Pin respects user intent.** If both X and Y are pinned and they contradict,
  repair has nothing to flip and bails. That's the desired behaviour - the user
  gets a clear error.
- **Rule order matters.** `solve` processes rules in declaration order. If
  repairing rule N tends to break rule M and vice versa, order them
  most-specific-first.
- **`re_fires_other_ok` short-circuits on the first re-firing rule.** It does
  not search for an alternative flip that wouldn't re-fire; that's left to the
  next disjunct.

## Inverse Transformations

[`schema/inverse.rs`](../../crates/codegen/src/schema/inverse.rs) is the
read-side counterpart to alias substitution. On read, ref-fields are converted
from wire to typed form _in topological convert order_; then the inverse pass
runs.

A transformation `T` is **invertible** iff:

- `T.when` is a single current-scope `Equals` leaf.
- No other transformation in the same list has the same `apply` set-pattern.

Reasoning:

- A single Equals leaf makes the "is this pattern on the wire" check unambiguous
  to write.
- A unique `apply` pattern makes it unambiguous to attribute - two
  transformations producing the same flat form can't be told apart on read.

For each invertible `T`, the emitted inverse:

```rust
if <all of T.apply's set-leaves are present in the profile> {
    profile.<T.when.ref> = Some(<T.when.equals>);
    // Clear every field touched by T.apply except T.when.ref.
    for a in T.apply where a.ref != T.when.ref:
        profile.<a.ref> = None;
}
```

Inverses run in **reverse** declaration order so chained aliases unwind
correctly: if you wrote them in order A->B, B->C, the read applies C->B then
B->A, getting back to the user-facing form.

Non-invertible transformations are skipped with a `cargo:warning` listing them.
That's the right behaviour because:

- An unconditional `apply` (e.g. defaulting padding to 0) has no meaningful
  inverse: you can't tell on read whether the user set 0 or the default kicked
  in.
- A compound `when` would need a multi-field check on the wire that may be
  ambiguous.

The intentional consequence: round-tripping a wire profile through deserialize
-> serialize is **not** guaranteed to be byte-identical if non-invertible
transformations modified the original. It is guaranteed to be _semantically_
equivalent (the camera will render the same image).

## Putting It All Together - The `try_update_from` Path

```
SimulationBase (partial)
    |
    | project into Self::default(), then per-field copy from partial
    v
partial_profile : <Camera>Simulation
    |
    | apply_transformations()  // value flattening
    v
partial_profile' : <Camera>Simulation
    |
    | generate_pin_set()       // record which fields the user set
    v
pin : HashSet<&'static str>
    |
    | candidate = self.clone()
    | candidate.<field> = Some(value) for each Some(value) in partial_profile'
    | candidate.apply_transformations()
    v
candidate : <Camera>Simulation
    |
    | candidate.solve(&pin)    // see Repair, above
    | candidate.emit_warnings_and_infos()
    v
*self = candidate
```

The render path is identical in shape but snapshots `original = self.clone()`
before the merge and passes it through `solve(&pin, &original)` and
`emit_warnings_and_infos(&original)`.

The `try_pull` (read) path is simpler:

```
for each field in read order:
    if gate(staged): staged.<field> = Some(read from PTP)
    else:            staged.<field> = None
```

The read path doesn't `solve`. If the camera returned an inconsistent state,
that's diagnostic information for a downstream write attempt - not something we
silently rewrite.
