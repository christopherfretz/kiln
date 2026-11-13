> **Frozen (WP-470, 2026-11-03): superseded by spec/reflection.md.** Kept as history.

# Reflection spike (WP-267, `-freflection`)

Status: spike landed behind `-freflection` (default off). It is a design
risk check for M6, not an implementation. The question it answers:
can a `std::meta` intrinsic **change sema state from inside a running Oven
evaluation**? Lookup, layout, codegen, instantiation order, the memo cache,
diagnostic attribution and memory accounting all have to keep working when
it does.

**Verdict: yes, with no architectural change.** The Oven/SemaBridge split
(spec/oven.md "Interface — amended 2026-10-28") held up. Every bend below
is local. One of them was a latent bug on main that any mutation (and
`define_aggregate` in particular) would hit: the Oven cached the layout of
a class that was still incomplete.

## What was built

| Piece | Where | Tests |
|---|---|---|
| `-freflection` driver/pp/lex/parse/sema option; `^^`, `[:`, `:]` lexed only with it (C++26) | `kiln_driver/src/args.rs:668`, `kiln_lex/src/lexer.rs:1368` | `reflection/flag-off.cpp` |
| `std::meta::info` value: builtin type `MetaInfo`, Oven `TyKind::Reflection` (8-byte scalar), freeze/thaw as `Scalar::Reflection`, NTTP, `==`/`!=`, `?:` | `kiln_oven/src/ty.rs:51`, `value.rs:368`, `kiln_sema/src/expr.rs`, `templ/nttp.rs` | `value-model{,-neg}.cpp` |
| `^^type-id`, `^^namespace`, `^^function`, `^^variable`, `^^S::member`; substitution in templates | `kiln_parse/src/reflect.rs:17`, `kiln_sema/src/reflect.rs:39`, `templ/subst/exprs.rs:541` | `reflect-operator.cpp` |
| Queries `type_of`, `name_of` (returns `const char*` into a frozen string), `members_of` (fixed-capacity `member_list<N>` built from `__kiln_members_count`/`__kiln_member_at`) | `tests/support/kiln_meta.h`; `kiln_sema/src/reflect.rs:167-330` | `queries{,-neg}.cpp` |
| Type splice `typename [: r :]` and `[: r :]` as a template argument | `kiln_parse/src/reflect.rs:73`, `kiln_sema/src/reflect.rs:109` | `splice-type{,-neg}.cpp` |
| **`__kiln_define_member(cls, type, name)`**: adds a public non-static data member to the class being defined | `kiln_sema/src/reflect.rs:330`, `kiln_oven/src/vm/meta.rs` | `define-member{,-neg,-run}.cpp`, `class.mem/incomplete-pointer-in-constant-evaluation.cpp` |

How an intrinsic is dispatched:
1. The compiler sees a direct call to a declaration that
   `SemaBridge::meta_intrinsic` recognizes by qualified name
   (`kiln_oven/src/compile.rs:4396`).
2. It emits `Op::Builtin` with aux `bi::META | MetaOp << 8`
   (`kbc.rs`, `compile.rs:4511`).
3. The VM's `exec_meta` (`vm/meta.rs`) decodes the operands and calls the
   bridge method, passing itself as `&mut Oven` so sema can evaluate in a
   nested evaluation.
4. A failure comes back as `Vec<Note>`. It becomes a trap: the first note
   is the message, the rest are extra notes, and the call stack goes in
   between.

Supported context for `__kiln_define_member`:
- The call must be manifestly constant-evaluated (sema's `eval_loc` is
  set), inside the member-specification of the innermost class being
  defined. A `static_assert` among the member declarations works, either
  directly or through a consteval helper.
- It works in a class template; each instantiation runs it.
- A speculative fold (`try_fold`) fails silently and never mutates.
- Contexts that are rejected, each with a note:
  - after the class is complete
  - from a nested class, naming the outer class
  - a non-class `cls`
  - an incomplete, function, reference or self `type`
  - a non-identifier name
  - a duplicate name, with three notes: the call, the previous
    declaration, and "in evaluation of ... for class 'S' being defined
    here"

**P2996's own rule differs.** `define_aggregate` *completes an
incomplete* class with all its members at once, from a consteval block
(spec/ast.md §10). The spike instead adds one member to a class
*mid-definition*. That is the harder mutation: there is no layout yet,
later members follow the injected one, and code earlier in the class has
already seen the class. So it covers the same risk. Real
`define_aggregate` adds one step on top of the same machinery: finishing a
forward-declared class from the evaluation.

## What worked unchanged
- **The SemaBridge boundary.** The Oven holds ids only. Sema appended a
  `Field` decl during the evaluation, and nothing the Oven held went
  stale. AST arenas and the reflection table are append-only, so the
  `ReflectionId`/`DeclId` values in Oven registers and frozen constants
  stay valid.
- **`DeclFlags::INJECTED`** and `append_member`'s permission to add a
  member to a class were already in kiln_ast (spec/ast.md §10).
- **Class completion.** It computes definition bits, aggregate-ness and
  layout at `}` from the member list, so injected fields are counted
  automatically: `sizeof`, `offsetof`, aggregate init and codegen
  (including the run test) needed no change.
- **Lookup.** `lookup_in` keeps no per-class cache that goes stale. A
  member injected in this evaluation is found by the next call in the
  same evaluation (the duplicate check), by later member function bodies,
  and by code after the class.
- **Instantiation.** `ClassBuild` is pushed for template instantiation
  as well, so "class being defined" means the specialization. Each
  specialization runs its own evaluation with its own `^^G`. On-demand
  completion (as a member of another class, or under `sizeof`) runs in
  demand order, which is deterministic.
- **Re-entrancy.** The lent-Oven pattern that `instantiate_for_oven`
  already used is the general answer for a bridge call that must
  evaluate (`require_complete` of a template specialization from inside
  `members_of`).
- **ConstStore.** `Scalar::Reflection` existed. So did the `MetaInfo`
  builtin type, `CXXReflectExpr`, and the pp `AvoidConcat` rules for the
  new tokens.
- **Accounting.** `-fconstexpr-report` attributes each defining
  evaluation to its `static_assert` line, or once per specialization at
  the template's line. Defining 20 members from one loop costs 104 steps
  and peak 8 bytes, with zero allocations left. The `oven:` budgets in
  `define-member.cpp` pin this. Sema-side work done inside a bridge call
  shows up only in `wall_us`, not in `steps`.
- **Forging is rejected.** `__builtin_bit_cast` to or from `info` is
  rejected (not trivially copyable), and so is a `memcpy` between `info`
  and an integer (typed copy check). No shadow bit was needed.

## What had to bend (file:line)
1. **Oven type table cached incomplete classes (latent bug on main).**
   `Types::record` built a class slot whenever the class had a
   definition, even one still being defined. It called
   `bridge.ensure_layout(def)`, and `kiln_abi::ensure_layout` caches by
   first declaration and is never invalidated. A `consteval f(S*)` called
   inside `S`'s member-specification therefore froze both the Oven's slot
   and sema's `ast.layouts` with the members seen so far. Clang accepts
   that program; on main kiln got `sizeof(S)` and aggregate init wrong.
   Fix: such a slot stays `Opaque`, is listed as provisional, and is
   rebuilt **in place** (same `OTy`, so pointers stay valid) once the
   class is complete. The rebuild happens on the next `intern` hit and at
   the start of every evaluation. See `kiln_oven/src/ty.rs:123,509-560`
   and `lib.rs:646`. Regression test:
   `class.mem/incomplete-pointer-in-constant-evaluation.cpp` (flag off).
2. **`ensure_layout` is only valid after `}`, but nothing enforced it**
   (`kiln_abi/src/layout/mod.rs:390`). The spike only avoids calling it
   (bend 1). See pre-M6 row R1.
3. **Memo cache.** Every meta intrinsic does `run.impure += 1`
   (`kiln_oven/src/vm/meta.rs:38`). `finish_memo` already skips any call
   whose span saw impurity, so the defining call and all its callers are
   never memoized. `define-member-neg.cpp` proves it: a second identical
   `add_x(^^S)` reports the duplicate instead of replaying the first
   result. Queries are impure too, because a class can gain members
   between evaluations.
4. **Mutation gate for speculation.** `define_member` and
   on-demand completion inside `members_of` return a silent failure
   unless `sema.inst.eval_loc` is set
   (`kiln_sema/src/reflect.rs:281,340`). Without this, a `try_fold` of
   the same expression (sema folds speculatively in several places) would
   mutate first. The Oven itself cannot tell "manifestly
   constant-evaluated" from a trial; that knowledge lives in sema.
5. **Entity identity.** `^^S` names the first declaration, while the class
   being defined is its definition. Comparing `DeclId`s directly failed
   for `struct S; struct S { ... }`, so the code compares `ast.first(..)`
   (`kiln_sema/src/reflect.rs:355`). Every reflection-to-entity comparison
   in M6 needs that rule.
6. **Bridge methods that evaluate take `oven: &mut Oven`.** These are
   `reflect_members` and `define_member` (`kiln_oven/src/lib.rs:321,340`).
   `lend_oven` was extracted from `instantiate_for_oven`
   (`kiln_sema/src/reflect.rs:421`, `oven_bridge.rs:198`). No `&` borrow
   had to become `&mut` otherwise: `meta_intrinsic` stays `&self`, because
   compile time only reads.
7. **Runtime traps carry sema notes.** `exec_meta` pushes a `TrapInfo`
   built from the bridge's notes at run time
   (`kiln_oven/src/vm/meta.rs`). Before, traps were compile-time stubs or
   fixed VM faults.
8. **Value encoding.** Arena ids are 1-based (`NonZeroU32`), but
   value-initialized `info{}` must be the null reflection. So the Oven
   value is `index - 1`, and table slot 1 is `Reflectee::Null`
   (`kiln_ast/src/reflect.rs:105`).
9. **Shadow memory has no kinds.** spec/oven.md promised "shadow kind
   `Reflection`". The real shadow has only init and pointer bits. The
   spike relies on typed loads and stores plus `memcpy`'s same-type check
   (see "Forging is rejected"). This is spec drift, now recorded in
   spec/oven.md.
10. **Lexer behaviour on main changed** (`kiln_lex/src/lexer.rs:1368`).
    `^^`, `[:` and `:]` were single tokens in every C++26 compile. They
    are now single tokens only under `-freflection`, matching clang and
    GCC 16. Without it, `^^` is `^` `^` and `[:` is `[` `:`.
11. **Visibility changes.** `expr_has_error`
    (`kiln_sema/src/templ/builtins/traits.rs:219`) and `srcloc_string`
    (`kiln_sema/src/builtins.rs:1572`) became `pub(crate)`; `bi_strlen`
    (`kiln_oven/src/vm/builtins.rs:284`) became `pub(super)`.
12. **Call-frame notes printed reflections as `^^#N`.** They now go
    through `PrintEnv::reflection` (`kiln_oven/src/notes.rs:753`).
13. **Language rule, not a bend.** In a class *template*, a member
    function must write `this->v` for an injected `v`, because unqualified
    non-dependent lookup happens at definition time ([temp.res]). Users
    will hit this. P2996 code uses splices instead.

## Known gaps (follow-ups, not design risks)
- **Alias sugar.** `^^Alias` canonicalizes. spec/ast.md §10 asks to keep
  the top-level alias (`kiln_sema/src/reflect.rs:39`).
- **Dependent splices** (`typename [: R :]` with `R` a template
  parameter) are reported as "unsupported: dependent splice". They need a
  `Splice` type node and substitution (`kiln_sema/src/reflect.rs:116`).
- **`using T = [: r :];` without `typename`** does not parse yet.
- **Expression splices** (`[: e :]` as an expression) are unsupported
  (`kiln_parse/src/expr.rs:1329`).
- **`members_of` cost.** `__kiln_members_count`/`__kiln_member_at`
  recompute the member list on every call, which is O(n²) for a loop.
  Real `members_of` returns a `std::vector<info>` from one bridge call.
  (Retired by WP-478: `members_of` is now the cached query of
  spec/reflection-members.md.)
- **Declaration location.** An injected member's location is the
  intrinsic call. Through a helper, "previous declaration is here"
  therefore points into the helper, not into the class.
- **Reflection table overflow.** `kiln_ast/src/reflect.rs:61`
  `ReflectionTable::intern` panics at 2^32 entries (`expect`). Make it a
  diagnostic.

## Proposed pre-M6 BACKLOG rows
- **R1 — layout of incomplete classes is unrepresentable**:
  `kiln_abi::ensure_layout` returns `Option` (or debug-asserts) for a
  class without `DefinitionBits::COMPLETE`, and every caller handles it.
  Today only the Oven's provisional slots keep it from happening, and
  `define_aggregate` adds more callers. (kiln_abi, kiln_oven, kiln_sema;
  small)
- **R2 — one "plainly constant-evaluated" flag**: sema's `eval_loc` gate
  (bend 4) becomes an explicit `EvalContext` bit the Oven carries
  (`run.manifest`), so bridge mutations ask the Oven rather than a sema
  side channel. It also feeds `consteval` blocks (P3289) and
  `is_constant_evaluated`. (kiln_oven, kiln_sema; small)
- **R3 — entity identity helper**: `Sema::same_entity(a, b)`, which
  compares first declarations, plus a lint/test that reflection code never
  compares `DeclId`s raw. (kiln_sema; small)
- **R4 — consteval blocks (P3289) and `define_aggregate` on an incomplete
  class**: use the spike's machinery, and add completion from the
  evaluation (`act_on_finish_record` on a forward-declared class) under
  provisional Oven slots. This proves the P2996 shape rather than the
  spike's. (kiln_parse, kiln_sema; medium)
- **R5 — spec/oven.md shadow kinds**: either add a 2-bit kind to the
  shadow (pointer / reflection / plain) or delete the promise and specify
  the typed-copy rule. (kiln_oven; small, decision first)

## Cost of full P2996 on top of this
These are estimates, in WP slots of roughly a week each:
- `<meta>` surface: about 60 functions, each a bridge method plus tests.
  4–6 slots. Most are table lookups like `type_of`.
- `std::vector<info>` / `std::string_view` returns: needs constant-evaluated
  libc++ `vector` with `info` elements (M3b) and a bridge call that fills
  an Oven allocation. 1–2 slots.
- Splices in every position (expression, member access, namespace,
  template, dependent), the `Splice` type node, and mangling. 4–6 slots.
- `define_aggregate`, `data_member_spec`, consteval blocks, the
  injection-point rules ([expr.const] "plainly constant-evaluated", scope
  checks). 2–3 slots (R2 and R4 first).
- `substitute`, `reflect_constant`/`extract`, `access_context`. 3–4 slots.
  `substitute` drives instantiation from the bridge; the lent-Oven pattern
  covers it.
- Expansion statements (P1306) and annotations (P3394). 3–4 slots.
- Errors as `std::meta::exception` (P3560) need constexpr exceptions
  (P3068) in the Oven. That is the largest unknown: 3–5 slots.

**Total: about 20–30 slots.** Constexpr exceptions and dependent splices
are the critical path. None needs an Oven/sema boundary change beyond
R1–R5.

## Open questions
1. Should `info` values keep shadow kind bits (R5), or is the typed-copy
   rule enough? This matters for `std::bit_cast` of aggregates that
   contain `info`.
2. Should reflection mangling (Itanium proposal) wait for the
   oracle/reference decision (GCC 16.1, BACKLOG WP-102)?
3. As I read P2996R13, `define_aggregate` targets a class that is
   incomplete, from an injection point outside its definition. It never
   adds to a class mid-definition. This needs confirming against the
   final wording. Should the spike intrinsic stay as a kiln extension
   (real code might want it), or be removed at M6?
