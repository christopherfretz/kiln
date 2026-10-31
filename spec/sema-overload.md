# spec: overload resolution (`kiln_sema::overload`)

Status: **partly implemented**: the core (named and member calls, ADL,
ICS, §4, initialization by constructor and conversion, default arguments,
§9.2 notes) landed with WP-068a (§10.5); operators and built-in candidates
are WP-068b. Designed by WP-S3O, split into 068a/068b (§11); WP-071 (function templates) and WP-073 (concepts) plug in through
§10.4. Normative for the `overload/` module of `kiln_sema`. Extends
spec/sema-conv.md (standard conversions, ranking) and spec/sema.md (lookup,
calls).

Standard: [over], [basic.lookup.argdep], [dcl.fct.default], [class.conv],
[expr.call], [expr.new]/[expr.delete] (lookup only). Reference: clang 19
`SemaOverload.cpp`. Every wording, note order and AST shape below was
checked with `clang++-19 -fsyntax-only` / `-Xclang -ast-dump`
(`--target=x86_64-linux-gnu -std=c++26`). Where the standard leaves a
choice kiln does what clang does; §9.3 lists the deviations.

## 1. Scope and principles

### 1.1 What M3 covers
All of [over.match] for non-template functions: named calls with ADL,
member calls, calls to class objects (with surrogates), operators (member,
non-member, built-in, rewritten, reversed), [over.match.ctor],
[over.match.copy], [over.match.conv], [over.match.ref], [over.match.list],
[over.over]. Implicit conversion sequences: standard (sema-conv.md §3),
user-defined, ellipsis, reference binding, list. [over.match.best] with
every tie-breaker; the template/constraint ones are hooks (§4.1) filled by
WP-071/073. Default arguments and `CXXDefaultArgExpr`. `operator new`/
`delete` lookup (§6.5). Out of scope: deduction itself
(spec/sema-templates.md), constraint satisfaction (WP-073), CTAD (hook
only, §2.8).

### 1.2 Principles
1. **One engine.** Every overloaded context runs `begin_set` → `add_*` →
   `best_viable` → `finish_*`/`diag_*` → `end_set`. No caller ranks.
2. **Failed candidates leave no AST.** ICS computation is a pure query
   (`try_implicit_conversion` returns an `Ics`, builds nothing, emits
   nothing). Only the winner's conversions are performed (`perform_ics`,
   §10.3), so hundreds of candidates in constexpr-heavy code cost no arena
   garbage. The same purity is what SFINAE (WP-071) needs.
3. **Clang order and shapes.** Candidates are inserted in clang's order
   (§2.1) so notes match; each resolved call kind has one AST shape (§8).

### 1.3 Interface with deduction (spec/sema-templates.md)
Overload resolution sees function templates only through these calls. S3T
owns the bodies (and may rename them) but must keep this contract:

```rust
// [temp.deduct.call]: on success the specialization is *declared* (no body,
// clang FinishTemplateArgumentDeduction); SFINAE errors are Err, undiagnosed.
fn deduce_call_candidate(&mut self, tmpl: DeclId, explicit: Option<TemplateArgsId>,
    args: &[ExprId], object: Option<ObjectArg>, ctx: DeduceCtx)
    -> Result<DeclId /*FunctionDecl specialization*/, DeductionFailure>;
fn deduce_conversion_candidate(&mut self, tmpl: DeclId, to: QualType)   // [temp.deduct.conv]
    -> Result<DeclId, DeductionFailure>;
fn deduce_address_candidate(&mut self, tmpl: DeclId, explicit: Option<TemplateArgsId>,
    target: QualType) -> Result<DeclId, DeductionFailure>;            // [temp.deduct.funcaddr]
// [temp.func.order]; None = neither more specialized.
fn more_specialized_function(&mut self, a: DeclId, b: DeclId, kind: PartialOrderKind,
    n_call_args: u32, reversed: bool) -> Option<DeclId>;
```

`DeductionFailure` is opaque here except `rank() -> u8` (clang
`RankDeductionFailure`, used by note ordering §9.2) and `note(&self, s,
cand) -> DiagBuilder`. The returned specialization becomes
`Candidate::decl`, the template `Candidate::template`. `ObjectArg` is the
object argument's `(QualType, ValueKind)`.

### 1.4 Performance
- **Pooled scratch.** `Sema.ovl: OverloadScratch { pool:
  Vec<OverloadCandidateSet> }`. `begin_set` pops a cleared set (or makes
  one); `end_set` clears it and pushes it back. Every Vec in a set keeps its
  capacity, so steady-state resolution allocates nothing. Nested
  resolutions (a constructor inside an ICS inside a call) take another set;
  pool depth = nesting depth.
- **ICS once per (candidate, argument)**, stored in the set's shared
  `ics: Vec<Ics>`; ranking, ambiguity marking and notes read it, nothing
  recomputes it. `Ics` is `Copy`, size asserted (≤ 96 bytes).
- **Arity first** (§3.1): before any ICS, and before deduction for
  templates. Conversion stops at the first `Bad` argument (clang does too;
  notes report only that one).
- **Lazy, memoized built-ins** (§6.2): generated only when an operand has
  class/enum type and the operator has built-in forms. The promoted
  arithmetic list is computed once per `Sema` (`BuiltinTables`,
  target-dependent). Per complete class, the pointer/member-pointer/enum
  types reachable via non-explicit conversion functions are memoized in
  `Sema.conv_type_cache: HashMap<DeclId, ConvTypeSet>`. Within one
  resolution `builtin_ics_cache` shares an operand's ICS to a type across
  candidates (144 relational pairs on x86_64 need 24 ICSs, not 288).
- **Accounting.** `OverloadScratch` and `conv_type_cache` implement
  `HeapSize`; `Sema::memory()` reports rows `sema.ovl.pool`,
  `sema.ovl.conv_types`. Bench counters (spec/bench.md): `ovl.sets`,
  `ovl.candidates`, `ovl.ics`, `ovl.builtin_candidates`, `ovl.max_set`.
- `lookup.rs` fills the set's `found` buffer instead of allocating a
  `Found { decls: Vec }` per lookup.

## 2. Candidate functions ([over.match.funcs], [over.match])

### 2.1 Named calls: when overload resolution runs
Clang resolves overloads only for an `UnresolvedLookupExpr` (ULE) or
`UnresolvedMemberExpr` (UME) callee; kiln does the same.
`act_on_id_expression` gains `has_trailing_lparen` from the parser and
builds a ULE when lookup found functions/function templates and either (a)
more than one decl or any template, or (b) the name is unqualified, followed
by `(`, and ADL is not suppressed (§2.2). Otherwise it builds the M2
`DeclRefExpr`. The ULE carries `requires_adl` (`ExprFlags`), the qualifier,
explicit template arguments and the found decls (an `IdList` in candidate
order).

`act_on_call` resolves a ULE/UME callee. Every other callee — `(g)(x)`,
`N::f(x)` with one `f`, a single member `h.g(x)`, function pointers — takes
the M2 direct path, whose errors use the initialization wording "cannot
initialize a parameter of type 'H *' with an rvalue of type 'int'" +
"passing argument to parameter here" (§13.2 item 3). `g(1)` with a single
unqualified `g` *does* resolve: "no matching function for call to 'g'".

A ULE outside a call goes to `resolve_address_of_overloaded` (§2.9) when
there is a target type; otherwise "reference to overloaded function could
not be resolved; did you mean to call it?" + one "possible target for call"
per candidate (a UME: "reference to non-static member function must be
called"; `auto x = f;`: "variable 'x' with type 'auto' has incompatible
initializer of type '<overloaded function type>'").

**Candidate order.** (1) Unqualified-lookup results reached through the
lexical scope chain, **newest first**; then (2) results from the
DeclContext table (other reopenings, using-directives) in declaration
order; then (3) ADL results not already present, by associated namespace in
discovery order, declaration order within. Qualified and member lookup:
declaration order. ast.md §4.3 stores declaration order, so `lookup_name`
reverses segment (1). Verified: `void f(int); void f(double); void(*p)(char)
= f;` notes `f(double)` before `f(int)`.

### 2.2 Argument-dependent lookup (`adl.rs`, [basic.lookup.argdep])
`adl_lookup(name, args, out)`. **Suppressed** (clang
`UseArgumentDependentLookup`) for qualified or parenthesized callees,
implicit builtins, or when ordinary lookup found a class member, a
block-scope function declaration (not a using-declaration), or anything
that is not a function or function template.

**Associated entities** (/3), `associated_entities(ty, &mut AssocSet)`:
pointers/references/arrays → element; function types → parameters and
return; member pointers → class and pointee; enum → innermost enclosing
namespace (+ class if a member); class → itself, all bases, their innermost
enclosing namespaces; template specializations add their template
arguments' entities; an overload-set (ULE) argument adds every candidate's.
**Lookup** searches each associated namespace's table directly, ignoring
using-directives, including inline namespaces both ways (`inline_children`
and the enclosing namespace), and finds friend functions declared in
associated classes, which are visible *only* here (S3C stores them with
`LookupFlags::ADL_ONLY`; ordinary lookup skips them). Dedup by canonical
decl. A winner found *only* by ADL sets the `adl` flag on the
`CallExpr`/`CXXOperatorCallExpr` (clang `IsADLCandidate`; dumped `adl`,
e.g. the hidden-friend `operator<` of a rewritten `<=>`).

### 2.3 Member calls ([over.match.call]/2)
`x.f(a)`, `p->f(a)`, implicit `this->f(a)` build a UME when member lookup
found several decls or a template; a single non-template member takes the
direct path (`MemberExpr`). Candidates in member-lookup order
(spec/sema-class.md): non-static members get the implicit object parameter
(§3.2); static members get one that accepts any object and is never
compared ([over.match.funcs]/4); explicit-object members take the object as
their first parameter; using-declared base members count as members of the
naming class (`found` = the `UsingShadowDecl`). An unqualified call whose
lookup found members uses `(*this)` when available; with no `this`, a
non-static winner is "call to non-static member function without an object
argument".

### 2.4 Call to an object of class type ([over.call.object])
`resolve_call_object(obj, args)`. Candidates: every `operator()` of `T`
(member lookup), then one **surrogate** per non-hidden, non-explicit
conversion function of `T` (and bases) to pointer/reference to function
whose cv-qualification is ≥ the object's: parameters `(conversion's object,
P1..Pn)`, `CandidateKind::Surrogate { conv }`, note "conversion candidate of
type 'void (*)(int)'". Neither present: "type 'H' does not provide a call
operator". Failure: "no matching function for call to object of type 'F'" /
"call to object of type 'F' is ambiguous".

### 2.5 Operators ([over.match.oper])
`resolve_operator(op, args, loc)`:
1. No operand of class or enum type (after reference stripping) → `None`,
   the M2 built-in path runs. `= [] () ->` resolve only for a class left
   operand.
2. **Member candidates**: qualified lookup of `operator@` in `T1` if complete
   or being defined.
3. **Non-member candidates**: unqualified lookup of `operator@` from the
   expression's context, skipping class scopes (/3.2), plus ADL over all
   operands.
4. **Built-in candidates** (§6.2), after 2–3; drop any whose
   parameter-type-list equals that of a non-member non-template candidate,
   rewritten ones included (/3.3.4). For built-in `=`/compound assignment
   the left operand admits no UDC and no temporary (/4):
   `IcsOptions::NO_UDC | NO_TEMPORARY`.
5. **Rewritten candidates** for `== != < > <= >= <=>` (§5).

Enum-only operands where 2–5 find no user candidate fall back to the M2
built-in path silently (as clang).

### 2.6 Initialization by constructor or conversion
| context | entry | candidates |
|---|---|---|
| [over.match.ctor] direct/default/value-init of class `T` | `resolve_ctor(T, args, Direct)` | all constructors (incl. inherited) |
| copy-init of `T` from `T` or derived | `resolve_ctor(T, [e], Copy)` | converting constructors |
| [over.match.copy] copy-init of `T` from class `S` not derived | `resolve_conversion(T, e, Copy)` | converting ctors of `T` + non-explicit conversion functions of `S`/bases yielding (cv) `T`, a derived class, or a reference to one |
| [over.match.conv] non-class `T` from class `S` | `resolve_conversion(T, e, ToNonClass)` | conversion functions yielding a type convertible to `T` by a standard conversion; explicit ones only in direct-init |
| [over.match.ref] reference to `cv2 T2` | `resolve_conversion(T, e, ToReference{lvalue})` | conversion functions yielding lvalue `cv2 T2&` (lvalue ref) or rvalue `cv2 T2`/`T2&&`, reference-compatible |
| [over.match.list] list-init of class `T` | `resolve_ctor(T, list, List)` | phase 1: initializer-list ctors with the list as sole argument; phase 2 (none viable): all ctors with the elements |

**`explicit`** (clang behaviour): in copy-init, explicit constructors and
conversion functions are added but fail with `CandidateFailure::Explicit`;
"explicit constructor is not a candidate" / "explicit conversion function is
not a candidate" notes appear only under "no viable conversion from …",
never in call-argument notes (there `tg(1)` with `explicit G(int)` just says
"no known conversion from 'int' to 'G' for 1st argument"). In
copy-list-init explicit ctors stay viable and choosing one is "chosen
constructor is explicit in copy-initialization" + "explicit constructor
declared here".

### 2.7 [over.best.ics]/4 restrictions
`IcsOptions::SUPPRESS_UDC`, passed by the nested resolution, forbids
user-defined conversions for the argument of a constructor's first
parameter (of class `X`) when the context is the second step of a class
copy-init, [over.match.copy]/[conv]/[ref], or [over.match.list] phase 2 with
a single initializer-list argument converting to `X` or `cv X&`. This also
breaks copy-ctor ↔ converting-ctor recursion.

### 2.8 CTAD hook
S3T builds the [over.match.class.deduct] guide set and resolves it with
`SetKind::Guides`/`CandidateKind::DeductionGuide` (§4.1 step 10); M3 only
reserves these.

### 2.9 Address of an overloaded function ([over.over])
`resolve_address_of_overloaded(ule, target, complain) -> Option<ExprId>`.
Targets: an initialized object or reference, assignment LHS, a parameter
(during ICS a ULE argument converts by [over.over]), a cast, a non-type
template parameter, a return. A non-template matches when its type equals
the target's function type after function-pointer conversion; non-static
members only through `&X::f` to a member pointer; templates via
`deduce_address_candidate`. Then: if any non-template matches, drop
templates; else keep the most specialized (`PartialOrderKind::Address`);
then the most constrained (WP-073). Still tied: "address of overloaded
function 'a' is ambiguous" + "candidate function [with T = int]" each. No
match: "address of overloaded function 'u' does not match required type
'void (long)'" + per candidate "candidate function has type mismatch at 1st
parameter (expected 'long' but has 'char')" or "candidate function has
different number of parameters (expected 1 but has 2)", in candidate order.
(A single non-overloaded `u` has no ULE; M2 says "cannot initialize a
variable of type 'void (*)(int, int)' with an lvalue of type 'void (int)':
different number of parameters (2 vs 1)".) AST: the ULE becomes the
selected `DeclRefExpr` (qualifier, `found` kept); `&f` is `UnaryOperator
'&'` over it; decay is `FunctionToPointerDecay` (`part_of_explicit_cast`
inside a cast).

## 3. Viable functions ([over.match.viable])

### 3.1 Arity
With `n` declared parameters (explicit object parameter included, implicit
excluded) and `m` arguments, viable only if `m == n`, or `m < n` and
parameters `m..n` all have defaults (merged view, §7), or `m > n` and
`FuncInfo::variadic`. Otherwise `TooFewArgs{min}` / `TooManyArgs{max}`;
notes "requires N argument(s), but M was/were provided" (`min == max`),
"requires at least …" / "requires at most …" otherwise. Then a trailing
*requires-clause* is checked through the WP-073 hook `constraints_satisfied`
(true until then) → `ConstraintsNotSatisfied`.

### 3.2 Object parameter ([over.match.funcs]/4–5)
The implicit object parameter is `cv X&` (no ref-qualifier or `&`) or `cv
X&&` (`&&`), `X` the naming class, `cv` from `FuncInfo::this_quals`. The
object argument is an lvalue for `x.f`, `p->f`, implicit `this`; a prvalue
for `S().f()`. `object_ics(obj_ty, obj_vk, method)` binds directly — no UDC,
no temporary (/5); a method without ref-qualifier lets an rvalue bind to
non-const `X&` (`RefBinding::OBJECT_NO_REF_QUAL`, ignored by /3.2.3); a
derived object is `DerivedToBase` (Conversion rank); an ambiguous base is
`Bad`. Notes: "'this' argument has type 'const S', but method is not marked
const"; "expects an lvalue for object argument" / "expects an rvalue for
object argument"; "no known conversion from 'X' to 'Y' for object
argument". An explicit object parameter is an ordinary first parameter
(full ICS, UDC allowed). A static member's object slot is identity with
`RefBinding::IGNORED`.

### 3.3 Implicit conversion sequences (`overload/ics.rs`)
```rust
pub enum Ics {                       // Copy
    Standard(StandardConversion),    // with ref-binding info, below
    UserDefined(UserConv),
    Ellipsis,
    Ambiguous { from: QualType, to: QualType },   // ranks as a UDC
    Bad(BadConv),
}
pub struct UserConv { before: StandardConversion, func: DeclId, found: DeclId,
    after: StandardConversion, via: UdcVia /*Ctor|ConvFn|Aggregate*/, list_ctor: bool }
pub struct BadConv { kind: BadKind, from: QualType, to: QualType }
pub enum BadKind { NoConversion, LvalueRequired, RvalueRequired, LosesQuals(Quals),
    BaseToDerived, InitList, ObjectQuals, Narrowing, ExplicitOnly, Incomplete }
```
`StandardConversion` (`conv/scs.rs`) gains `rb: RefBinding` (u8 flags
`IS_REF DIRECT LVALUE_REF BINDS_RVALUE BINDS_FUNCTION_LVALUE
OBJECT_NO_REF_QUAL IGNORED`) and `SecondStep::DerivedToBase` (class to base
by value or reference; Conversion rank, [over.best.ics]/6).
`try_implicit_conversion(arg: ArgInfo, to, opts: IcsOptions) -> Ics` is
pure. `IcsOptions`: `SUPPRESS_UDC`, `NO_UDC`, `NO_TEMPORARY`,
`ALLOW_EXPLICIT` (direct-init, contextual bool).

**[over.ics.ref]** (target `cv1 T1&`/`&&`): (1) direct binding
(reference-compatible with the right value category) → identity,
`DerivedToBase` or qualification/function-pointer adjustment, `rb =
IS_REF|DIRECT|…`; (2) class `T2` → [over.match.ref] conversion functions,
`UserDefined` with `rb` on `after`; (3) else a non-const lvalue reference →
`Bad(LvalueRequired)` ("expects an lvalue for 1st argument"), `&&` to an
lvalue → `Bad(RvalueRequired)`, otherwise bind to a temporary: ICS of `e →
T1` with `rb = IS_REF` (not `DIRECT`). Dropping cv →
`Bad(LosesQuals)` ("1st argument ('const int *') would lose const
qualifier").

**[over.ics.user]**: same or derived class source to class `T` → standard
(identity/`DerivedToBase`, no constructor in the ranking); other class
target → nested `resolve_conversion(T, e, Copy)` with `SUPPRESS_UDC` on its
arguments (winner → `UserConv`, tie → `Ics::Ambiguous`); non-class target
from class → [over.match.conv]. **Ellipsis**: arguments past `n` of a
variadic candidate; non-trivial classes are still `Ellipsis` and are
diagnosed only when performed ("cannot pass object of non-trivial type 'NT'
through variadic function; call will abort at runtime", `-Wnon-pod-varargs`,
error by default).

**[over.ics.list]** (`InitListExpr` argument):
| target | ICS |
|---|---|
| `std::initializer_list<X>` (or ref) | worst element ICS to `X`; identity if empty; any Bad → `Bad(InitList)` |
| array `X[N]`/`X[]` | worst element ICS; too many elements → Bad |
| non-aggregate class | `UserDefined` from [over.match.list] (`list_ctor` if phase 1 won) |
| aggregate class | `UserDefined{via: Aggregate}` if aggregate init is well-formed |
| reference | as the referenced type, bound to the temporary |
| non-class, one element / empty | element's ICS (narrowing → `Bad(Narrowing)`) / identity |

Wording: "cannot convert initializer list argument to 'int'".

### 3.4 Comparing ICSs (`compare_ics`, [over.ics.rank])
(1) /2: `Standard` < `UserDefined` = `Ambiguous` < `Ellipsis`. (2) /3.1
list sequences: to `std::initializer_list<X>` beats not; for arrays, the
smaller (known) bound wins. (3) /3.2 two standard sequences:
`compare_standard_conversions` extended with /3.2.3 (both direct reference
bindings, neither `OBJECT_NO_REF_QUAL`: rvalue ref binding an rvalue beats
lvalue ref), /3.2.4 (lvalue ref to function lvalue beats rvalue ref), /3.2.6
(references differing only in top-level cv: less-qualified referee wins).
Rule order is clang's `CompareStandardConversionSequences`: subset, rank
(with /4.1 bool and /4.2 fixed enum), void*/derived pointer part of /4.4,
qualification /3.2.5, /3.2.3–3.2.4, /4.3, derived-to-base /4.4, /3.2.6;
sema-conv.md §4 is rewritten to that order (§13.2 item 1). (4) /3.3 two
UDCs: comparable only with the same conversion function/constructor (or
both aggregate); then compare `after`; else indistinguishable.

## 4. Best viable function (`overload/best.rs`, [over.match.best])

### 4.1 `better(set, c1, c2)`, C++26 order
Over argument slots `k` (skipping `RefBinding::IGNORED`; reversed
candidates in parameter order), `c1` is better iff no `ics1[k]` is worse
than `ics2[k]` and the first applicable of:
1. /2.1 some `ics1[k]` is better;
2. /2.2 UDC context ([over.match.conv/copy/ref]): `c1`'s return-to-
   destination standard conversion (`Candidate::final_conv`) is better;
3. /2.3 [over.match.ref] function-reference binding: `c1` returns the same
   reference kind as the target, `c2` does not;
4. /2.4 `c1` non-template, `c2` a template specialization;
5. /2.5 both specializations and `more_specialized_function` picks `c1`'s
   template (WP-071; `None` until then);
6. /2.6 both non-template, same parameter-type-lists (after reversal, object
   parameters included) and `more_constrained(c1, c2)` (WP-073; false until
   then);
7. /2.7 `c1` constructor of `D`, `c2` inherited from base `B`, same
   parameter types;
8. /2.8 `c2` rewritten, `c1` not;
9. /2.9 both rewritten, `c2` reversed, `c1` not;
10. /2.10–2.13 CTAD guides (§2.8): inherited-constructor guides; declared
    guide beats generated; copy deduction candidate; non-template-ctor guide
    beats template-ctor guide.

Clang extras: reversed-operator acceptance kept (§5.3); the rest dropped (§9.3).

### 4.2 Selection
```rust
fn best_viable(&mut self, set: &mut OverloadCandidateSet) -> OvlOutcome {
    let mut best = None;                                      // clang's two passes
    for c in set.viable() { if best.map_or(true, |b| self.better(set, c, b)) { best = Some(c) } }
    let Some(b) = best else { return OvlOutcome::NoViable };
    if set.viable().any(|c| c != b && !self.better(set, b, c)) {
        set.mark_ambiguous(|c| set.viable().all(|d| !self.better(set, d, c)));
        return OvlOutcome::Ambiguous;
    }
    if self.is_deleted(set[b].decl) { OvlOutcome::Deleted(b) } else { OvlOutcome::Success(b) }
}
```
After `Success`, in order: access check on `found` (S3C
`check_member_access`: "'p' is a private member of 'Q'" + "declared private
here"; constructors: "calling a private constructor of class 'P'");
`[[deprecated]]`/`[[nodiscard]]` (sema.md §4); odr-use; `perform_ics` per
argument — a selected `Ics::Ambiguous` is diagnosed here: "conversion from
'B' to 'A' is ambiguous" + the tied UDC candidates ("candidate
constructor", "candidate function").

## 5. Rewritten comparison candidates ([over.match.oper]/3.4, P2468)

### 5.1 Generation (`oper.rs::add_rewritten`)
The §2.5 lookups are repeated for the names below; each hit adds a
candidate with `rewrite: Rewritten` (operands `x, y`) or `Reversed`
(operands `y, x`). Built-ins are never rewritten.

| `@` | rewritten | reversed |
|---|---|---|
| `==` | — | `operator==` |
| `!=` | `operator==` | `operator==` |
| `< > <= >=` | `operator<=>` | `operator<=>` |
| `<=>` | — | `operator<=>` |

**P2468**: an `operator==` `F` yields no rewritten/reversed candidate if
searching `operator!=` in the scope where `F` was found (its class for
members, its namespace otherwise; no ADL) finds a function corresponding to
`F` ([basic.scope.scope]/4). Verified: with a matching `operator!=`, `a ==
a` on a non-const member `operator==` has no reversed candidate and no
warning. A selected rewritten `==` must return cv `bool`, else clang's
`err_ovl_rewrite_equalequal_not_bool` (wording copied by 068b).

### 5.2 Building the result
- `x != y`: `CXXRewrittenBinaryOperator 'bool'` → `UnaryOperator '!'` → the
  `==` call.
- Reversed `x == y`: `CXXRewrittenBinaryOperator` → `CXXOperatorCallExpr
  '=='` with arguments in **parameter** order (`y, x`); its range is first
  argument's begin to last argument's end, possibly inverted (`<col:45,
  col:40>`, as clang).
- `x < y` via `<=>`: `CXXRewrittenBinaryOperator` → the result of a nested
  `resolve_operator(<, [x <=> y, 0])` (reversed: `[0, y <=> x]`), i.e. a
  `CXXOperatorCallExpr '<'` or built-in `BinaryOperator`. The `0` is
  `IntegerLiteral 'int' 0` at the operator location, converted to the
  parameter (for `std::strong_ordering`'s comparisons,
  `<NullToPointer>`). Its failure: "invalid operands to binary expression
  ('int' and 'std::strong_ordering')" + "while rewriting comparison as call
  to 'operator<=>' declared here".

Only the semantic form is stored and dumped (consistent with ast.md §5.6).

### 5.3 Reversed ambiguity (clang extension, kept)
Under C++20 rules `a == a` with non-const member `bool operator==(const
P1&)` is ambiguous between normal and reversed. Clang
(`isBetterOverloadCandidate`) prefers the non-reversed candidate when both
have the same parameter types (or are the same function) and each is better
for some argument; kiln mirrors that check and clang's
`-Wambiguous-reversed-operator` warning: "ISO C++20 considers use of
overloaded operator '==' (with operand types 'P1' and 'P1') to be ambiguous
despite there being a unique best viable function" + "ambiguity is between
a regular call to this operator and a call with the argument order
reversed" + (non-const member only) "mark 'operator==' as const or add a
matching 'operator!=' to resolve the ambiguity".

## 6. Operator overloading semantics

### 6.1 Declarations ([over.oper], `check_operator_decl`)
`= () [] ->` and conversion functions must be non-static members (except
C++23 `static operator()`/`operator[]`; `new`/`delete` are implicitly
static). Others: members, or non-members with a class/enum (reference)
parameter. Arity per [over.unary]/[over.binary]; `()`/`[]` any count
(C++23 multi-arg `[]`); postfix `++`/`--` take a second `int`; no default
arguments except for `()`/`[]`. Wordings are clang's
`err_operator_overload_*` (e.g. "overloaded 'operator+' must be a unary or
binary operator (has 3 parameters)"), each pinned by a 068b test. The
`DeclName` operator tag (ast.md §3) is the lookup key.

### 6.2 Built-in candidates ([over.built], `builtin.rs`)
Per operand, a `BuiltinTypeSet` is built from the operand's type and its
class's memoized `ConvTypeSet`: pointer types (with the cv variants
[over.built] needs), member pointers, enums, `has_arithmetic`,
`has_nullptr`. Candidates are emitted per [over.built]/3–25 in **clang's
`BuiltinOperatorOverloadBuilder` order**, which fixes note order. Promoted
arithmetic order: float, double, long double, [`__float128` where
`TargetTypes` has it — x86_64 yes, aarch64 no], int, long, long long,
__int128, unsigned int, unsigned long, unsigned long long, unsigned
__int128; binary arithmetic/relational take every L×R pair (`a < a`: 144
notes on x86_64, 121 on aarch64). A built-in is
`CandidateKind::Builtin{params, n}` with no decl, noted "built-in candidate
operator+(float, int)".

**Selected built-in**: class operands are converted by the UDC part only
(`UserDefinedConversion` over `CXXMemberCallExpr`), *not* the `after`
sequence ([over.match.oper]/11, clang `CCK_ForBuiltinOverloadedOp`); then
the M2 built-in path builds a plain `BinaryOperator`/`UnaryOperator`/
`ArraySubscriptExpr`. **Failures**: "invalid operands to binary expression
('B' and 'int')" (notes only for user-declared non-viable candidates);
"invalid argument type 'B' to unary expression"; "use of overloaded
operator '+' is ambiguous (with operand types 'A' and 'int')" with "because
of ambiguity in conversion of 'A' to 'float'" (when an ICS is ambiguous),
"candidate function" and "built-in candidate …" notes; "overload resolution
selected deleted operator '='"; "no viable overloaded '='" + "candidate
function (the implicit copy assignment operator) not viable: …".

### 6.3 `()`, `[]`, `++`/`--`
`obj(args)`: §2.4. `a[b...]`: member `operator[]` with all arguments;
built-ins only for exactly one argument; "no viable overloaded operator[]
for type 'G'", or "type 'H' does not provide a subscript operator" when
there is no `operator[]` and no built-in is viable. Postfix `x++` calls with
`(x, 0)`, the `0` an `IntegerLiteral 'int' 0` with **invalid** location (as
clang dumps it). C++23 static `operator()`/`[]`: the object is still
evaluated; shape as clang 19's dump, pinned by a 068b golden (§13 q8).

### 6.4 `->` chains ([over.ref])
`resolve_arrow(base)`: while the base is a class, resolve member
`operator->` (no arguments; members only, no built-ins), build
`CXXOperatorCallExpr '->'`, continue on its result. Visited canonical
classes are kept in a small Vec: a repeat is "circular pointer delegation
detected" + per step "'operator->' declared here produces an object of type
'C'". Depth limit 256 (`-foperator-arrow-depth=N`), clang's
`err_operator_arrow_depth_exceeded` + `note_operator_arrow_depth`. The final
base must be a pointer: `MemberExpr ->m` over `ImplicitCastExpr <NoOp>` (cv
added as needed) over the last call.

### 6.5 `operator new`/`operator delete` lookup (interface for WP-061a)
```rust
pub struct AllocFns { pub new_fn: DeclId, pub delete_fn: Option<DeclId>,
                      pub pass_align: bool, pub sized_delete: bool }
fn find_allocation_functions(&mut self, ty: QualType, array: bool, global_only: bool,
    placement: &[ExprId], loc: SourceLoc) -> Option<AllocFns>;
fn find_deallocation_function(&mut self, ty: QualType, array: bool, global_only: bool,
    loc: SourceLoc) -> Option<DeallocFn /*decl, sized, aligned, destroying*/>;
```
Allocation ([expr.new]/12–19): for a class (element) type and no `::`,
member lookup of `operator new`/`new[]` in `T`, else global; arguments
`(size_t, placement...)` through `resolve_call` (callee rendered 'operator
new'); for over-aligned `T` (`alignof > __STDCPP_DEFAULT_NEW_ALIGNMENT__`)
first with `std::align_val_t` as argument 2, then without. Failure: "no
matching function for call to 'operator new'". Deallocation (the matching
delete of a new-expression, and [expr.delete]/10) is a rule-based pick, not
[over.match]: destroying, then alignment, then sized vs unsized, as clang
`resolveDeallocationOverload`. The global replaceable forms
([basic.stc.dynamic]/2) are declared lazily on first use
(`declare_global_new_delete`, clang `DeclareGlobalNewDelete`). WP-061a uses
these for non-class types; 068b adds the class-member and placement paths.

### 6.6 Conversion functions and `explicit` ([class.conv.fct], [conv]/5–6)
Candidates are the conversion functions (templates via
`deduce_conversion_candidate`) of the class and bases, not hidden; hiding
is by converted-to type, so lookup keys on the `DeclName` conversion tag
plus canonical type. `explicit` ones apply to direct-init, explicit casts
and contextual bool. `contextually_convert_to_bool(e)` becomes
`resolve_conversion(bool, e, ToNonClass, ALLOW_EXPLICIT)` (as `bool t(e)`):
`if (w)` accepts `explicit operator bool`, `bool k = w;` is "no viable
conversion from 'W' to 'bool'" + "explicit conversion function is not a
candidate". Contextual implicit conversion ([conv]/6: `switch`, `delete`,
array-new size) is `contextual_implicit_convert(e, pred)`: the unique
non-explicit conversion function whose result satisfies `pred`; several is
clang's `err_*_multiple_conversions` + one note per function.

## 7. Default arguments ([dcl.fct.default])
`ParamData.default_arg: Option<ExprId>` (ast.md §16.10), write-once.
**Ownership**: parsing is WP-065 (in place at namespace and block scope,
`DefaultArg::Parsed` in `ParamInfo`; saved and replayed in complete-class
contexts for member functions, `DefaultArg::Unparsed` → `ParamData::
DEFAULT_ARG_PENDING`, sema-class.md §2.7); sema
(`act_on_param_default_arg(p, e)`, the checks and merging below) is
WP-066b, temporaries and cleanups in default arguments WP-067b.
**Redeclarations**
(`merge_default_args(new, old)` from redeclaration merging): a same-scope
redeclaration may add defaults to parameters without one, and shares the
old `ExprId` with `ParamFlags::INHERITED_DEFAULT`; giving one again is
"redefinition of default argument" + "previous definition is here"; a
parameter without a default after one with a default (after merging) is
"missing default argument on parameter 'b'"; a block-scope redeclaration
starts fresh (/4); friend declarations with defaults must be definitions;
out-of-class members of class templates may not add defaults. **Calls**:
arity (§3.1) uses the called decl, i.e. the most recent visible
redeclaration. `build_call_args` appends per missing argument a
`CXXDefaultArgExpr` (parameter type, prvalue, `<<invalid sloc>>` in the
dump) with payload `(param: DeclId, used_ctx: DeclContextId)`; the dump
shows the default expression as its child, which is shared, never cloned.
Temporaries in a default make the full-expression an `ExprWithCleanups`
(WP-067). The Oven evaluates the shared expression with no `SemaBridge`
change (it is already resolved). **Instantiation hook (S3T)**: a default
argument of a class-template member or function-template specialization is
instantiated on first use, `instantiate_default_arg(fn, param: u16) ->
Option<ExprId>`, cached in the specialization's `ParamData`; errors note
"in instantiation of default function argument expression for 'f<int>'
required here". Virtual calls use the static type's (selected decl's)
defaults.

## 8. Resulting AST (dump goldens must match clang)
| resolved call | shape |
|---|---|
| free function | `CallExpr` → `ImplicitCastExpr <FunctionToPointerDecay>` → `DeclRefExpr` (qualifier kept; `adl` if ADL-only) |
| member, implicit object | `CXXMemberCallExpr` → `MemberExpr '<bound member function type>' .m`/`->m` → object; const method: object wrapped in `ImplicitCastExpr 'const S' lvalue <NoOp>` |
| member, explicit object | `CallExpr` → `ImplicitCastExpr 'int (*)(const Y &, int)' <FunctionToPointerDecay>` → `DeclRefExpr CXXMethod`; object is argument 0, converted |
| static member via object / `Y::s` | `CallExpr` → `MemberExpr 'int (int)' lvalue .s` / qualified `DeclRefExpr` |
| operator | `CXXOperatorCallExpr '<op>'` → `ImplicitCastExpr <FunctionToPointerDecay>` → `DeclRefExpr 'operator<op>'` (range = operator tokens); operands in parameter order, object first (NoOp to `const T` lvalue for const methods) |
| `()` / `[]` / `->` | `CXXOperatorCallExpr '()'`/`'[]'`/`'->'`, all arguments after the object |
| surrogate | `CallExpr` whose callee is `ImplicitCastExpr <UserDefinedConversion>` → `CXXMemberCallExpr` of the conversion |
| built-in candidate | plain `BinaryOperator`/`UnaryOperator`/`ArraySubscriptExpr` over UDC-converted operands (§6.2) |
| rewritten / reversed | §5.2 |
| conversion function | `ImplicitCastExpr '<T>' <UserDefinedConversion>` (lvalue for reference results) → `CXXMemberCallExpr` → `MemberExpr .operator T`; in `static_cast`: `CXXStaticCastExpr <NoOp>` over it, marked `part_of_explicit_cast` |
| converting ctor (copy-init, by-value arg) | `ImplicitCastExpr 'S' <ConstructorConversion>` → `CXXConstructExpr 'S' 'void (int)'` → argument |
| `const S&` bound to a temporary | `MaterializeTemporaryExpr 'const S' lvalue` → `CXXConstructExpr 'const S'`; full-expression in `ExprWithCleanups` (nodes owned by WP-067) |
| class by value from class | `CXXConstructExpr 'Z' 'void (const Z &) noexcept'` → `ImplicitCastExpr 'const Z' lvalue <NoOp>` |
| list-init | `CXXConstructExpr 'S' 'void (int)' list` |
| address of overload / default arg | §2.9 / `CXXDefaultArgExpr <<invalid sloc>> 'int'` (§7) |

`DeclRefExpr`/`MemberExpr` keep `found_decl` (the `UsingShadowDecl` when one
was used). Type sugar (`'Z':'N::Z'`) is the printer's job (ast-dump.md).

## 9. Diagnostics

### 9.1 New diagnostic ids (`kiln_diag/src/ids.rs`, clang wording)
| id | text |
|---|---|
| `sema_ovl_no_viable_member_function_in_call` | "no matching member function for call to '{0}'" |
| `sema_ovl_ambiguous_call` / `_member_call` | "call to '{0}' is ambiguous" / "call to member function '{0}' is ambiguous" |
| `sema_ovl_deleted_call` / `_member_call` | "call to deleted function '{0}'" / "call to deleted member function '{0}'" |
| `sema_ovl_no_viable_object_call` / `_ambiguous_object_call` | "no matching function for call to object of type '{0}'" / "call to object of type '{0}' is ambiguous" |
| `sema_ovl_no_viable_ctor` / `_ambiguous_ctor` / `_deleted_ctor` | "no matching constructor for initialization of '{0}'" / "call to constructor of '{0}' is ambiguous" / "call to deleted constructor of '{0}'" |
| `sema_ovl_no_viable_oper` / `_subscript` | "no viable overloaded '{0}'" / "no viable overloaded operator[] for type '{0}'" |
| `sema_ovl_ambiguous_oper_binary` / `_unary` | "use of overloaded operator '{0}' is ambiguous (with operand types '{1}' and '{2}')" / "… (with operand type '{1}')" |
| `sema_ovl_deleted_oper` | "overload resolution selected deleted operator '{0}'" |
| `sema_ovl_no_conversion` / `_ambiguous_conversion` | "no viable conversion from '{0}' to '{1}'" / "conversion from '{0}' to '{1}' is ambiguous" |
| `sema_no_call_operator` / `sema_no_subscript_operator` | "type '{0}' does not provide a call operator" / "… a subscript operator" |
| `sema_ovl_unresolvable` / `sema_member_must_be_called` | "reference to overloaded function could not be resolved; did you mean to call it?" / "reference to non-static member function must be called" |
| `sema_addr_ovl_no_match` / `_ambiguous` | "address of overloaded function '{0}' does not match required type '{1}'" / "address of overloaded function '{0}' is ambiguous" |
| `sema_explicit_ctor_copy_list_init`, `sema_member_call_without_object`, `sema_circular_arrow` | §2.6, §2.3, §6.4 |
| `warn_ambiguous_reversed_operator` [ambiguous-reversed-operator] + 2 notes | §5.3 |
| notes | `note_ovl_candidate` "candidate {kind}{tmpl}" with `_not_viable` ("… not viable: {detail}"), `_deleted` ("candidate function has been explicitly deleted"), `_explicit`, `_type_mismatch`; `note_ovl_builtin_candidate` "built-in candidate {0}"; `note_ovl_surrogate` "conversion candidate of type '{0}'"; `note_ambiguous_conversion_of`; `note_possible_target` "possible target for call"; `note_explicit_ctor_here`; `note_arrow_produces`; `note_rewriting_operator_as_spaceship` |

`{kind}` (clang `note_ovl_candidate` select): "function", "function (with
reversed parameter order)", "constructor", "constructor (the implicit
default constructor)" / "(… copy constructor)" / "(… move constructor)",
"function (the implicit copy assignment operator)" / "(… move assignment
operator)", "function (the implicit 'operator==' for this 'operator<=>)'",
"inherited constructor"; `{tmpl}` adds " template" / " [with T = int]".
`{detail}`: "no known conversion from 'X' to 'Y' for Nth argument"
(reference parameters print the referee, 'const H'); "expects an lvalue
for Nth argument"; "expects an rvalue for Nth argument"; "Nth argument
('const int *') would lose const qualifier"; "cannot convert from base
class pointer 'B *' to derived class pointer 'D *' for Nth argument";
"cannot convert initializer list argument to 'int'"; "'this' argument has
type 'const S', but method is not marked const"; "requires …" (§3.1); and
the "for object argument" forms. Existing `sema_ovl_no_viable_function_in_call`
and `note_ovl_candidate_not_viable` are reused; `sema_overload_unsupported`
is deleted when 068a lands.

### 9.2 Which candidates are noted, in what order
**Selection** (clang `CompleteCandidates`): `NoViable` and `Deleted` note all
candidates; `Ambiguous` notes the `mark_ambiguous` set; non-viable built-ins
are never noted; no cap (clang's default `-fshow-overloads=all`).

**Order**: a *stable* sort by `compare_for_display` (clang
`CompareOverloadCandidatesForDisplay`):
1. viable before non-viable; viable ones by ICS (better first), then
   location;
2. non-viable arity failures go last; among them smaller `|n − m|` first;
   equal distance, different kinds: too-many before too-few; else keep
   insertion order;
3. bad conversions (by number of bad conversions, then `compare_ics` on the
   first bad argument) before bad deductions (by `DeductionFailure::rank`);
4. then source location;
5. location-less candidates (built-ins) keep insertion order at the end of
   their group.

Verified: deleted best, distance 1, two distance-3 (scope order), distance 4.

**Locations**: the primary diagnostic is at the callee name (member name
for member calls), the operator token, the declarator / `T(` for
constructors, the expression for conversions; notes are at each candidate's
name; implicit special members at the class name.

### 9.3 Deviations from clang 19 (authoritative)
1. `-fshow-overloads=best` unsupported (always `all`).
2. No `enable_if`/`diagnose_if`, CUDA, `target` multiversioning, ObjC, or C
   `__attribute__((overloadable))`.
3. No fix-it hints on candidate notes.
4. No typo correction of unresolved callees (sema.md).

## 10. Interfaces

### 10.1 Types (`overload/mod.rs`)
```rust
pub struct OverloadCandidateSet {
    pub kind: SetKind,  // Normal | Operator(OverloadedOperator) | CopyInit | ConvToNonClass
                        // | ConvToRef{lvalue} | ListCtor{phase} | Address | Guides
    pub loc: SourceLoc,
    pub args: Vec<ArgInfo>,          // (ExprId, QualType, ValueKind, is_init_list), once
    pub object: Option<ArgInfo>,
    pub cands: Vec<Candidate>,
    pub ics: Vec<Ics>,               // indexed by Candidate::ics
    pub builtin_ics_cache: Vec<(u8, TypeId, u32)>,
    pub found: Vec<DeclId>,          // lookup buffer, also dedup
}
pub struct Candidate {
    pub decl: Option<DeclId>,        // None for built-ins
    pub found: DeclId,               // UsingShadowDecl or decl
    pub template: Option<DeclId>,    // WP-071
    pub kind: CandidateKind,         // Function | Method | Ctor | ConvFn | Surrogate{conv}
                                     // | Builtin{params: [QualType; 3], n: u8} | DeductionGuide
    pub ics: Range<u32>,             // object slot first when present
    pub viable: bool, pub failure: CandidateFailure,
    pub rewrite: Rewrite,            // None | Rewritten | Reversed
    pub adl_only: bool, pub ambiguous_mark: bool,
    pub final_conv: Option<StandardConversion>,   // /2.2
}
pub enum CandidateFailure { None, TooManyArgs{max: u16}, TooFewArgs{min: u16},
    BadConversion{arg: u16 /*u16::MAX = object*/}, BadDeduction(DeductionFailure),
    ConstraintsNotSatisfied, Explicit, BadFinalConversion, IllFormedRewrite }
pub enum OvlOutcome { Success(u32), NoViable, Ambiguous, Deleted(u32) }
```
Deleted candidates are viable; access is checked after selection.

### 10.2 Entry points (`Sema`) and `AstBuilder` constructors
```rust
fn resolve_call(&mut self, callee: ExprId /*ULE|UME*/, args: &[ExprId], rparen: SourceLoc) -> ExprId;
fn resolve_call_object(&mut self, obj: ExprId, args: &[ExprId], rparen: SourceLoc) -> ExprId;
fn resolve_operator(&mut self, op: OverloadedOperator, args: &[ExprId], loc: SourceLoc,
    range: SourceRange) -> Option<ExprId>;           // None ⇒ caller's built-in path
fn resolve_arrow(&mut self, base: ExprId, loc: SourceLoc) -> ExprId;
fn resolve_ctor(&mut self, ty: QualType, args: &[ExprId], kind: CtorInitKind
    /*Direct|Copy|List|Default|Value*/, range: SourceRange) -> ExprId;
fn resolve_conversion(&mut self, to: QualType, e: ExprId, kind: ConvInitKind,
    opts: IcsOptions) -> Result<UserConv, ConvError>;   // query; performed by perform_ics
fn resolve_address_of_overloaded(&mut self, ule: ExprId, target: QualType,
    complain: bool) -> Option<ExprId>;
```
`pub(crate)` primitives for WP-071/073/075: `add_function`, `add_method`,
`add_template`, `add_conversion`, `add_surrogate`, `add_builtin_candidates`,
`add_rewritten`. New `AstBuilder` constructors (ast.md §5.3 payloads; each
computes type, value kind and dependence like the existing ones):
`unresolved_lookup(name, qualifier, decls, explicit_args, requires_adl)`,
`unresolved_member(base, arrow, name, decls, explicit_args)`,
`member_call(callee_member, args, ty, vk, rparen)`,
`operator_call(op, callee, args, ty, vk, range, adl)`,
`rewritten_binary(semantic)`, `construct(ty, ctor, args, flags /*list,
elidable, zero_init*/, range)`, `default_arg(param, used_ctx)`. The ULE/UME
exist only until resolution; dependent ones (WP-070) persist.

### 10.3 What `implicit_convert` becomes
Its signature stays (sema-conv.md §5); the body becomes:
```rust
let ics = self.try_implicit_conversion(arg_info(e), to, opts_for(ctx));
match ics { Ics::Bad(b) => Err(b.to_conv_error(ctx)),
            Ics::Ambiguous{..} => { self.diag_ambiguous_conversion(e, to); Err(..) }
            _ => Ok(self.perform_ics(e, &ics, to, ctx)) }
```
`perform_ics` builds SCS via `apply_standard_conversion`, a UDC as `before`
+ `CXXConstructExpr`/conversion call + `after`, and a temporary binding via
`MaterializeTemporaryExpr`. sema-conv.md §5's "unsupported" class-target
and class-source branches go away; the direct path's `ConvCtx::Argument`
wording is unchanged. `has_constructors_or_unknown` is replaced by the S3C
class query, and `perform_init` (sema.md §6) calls
`resolve_ctor`/`resolve_conversion` for class and reference targets.

### 10.4 Extension points
- **WP-071**: `add_template` calls `deduce_call_candidate` (§1.3) after the
  arity check; `Err` (SFINAE) becomes `BadDeduction`; §4.1 steps 4–5 call
  `more_specialized_function`; explicit template args ride on the ULE/UME.
- **WP-073**: `constraints_satisfied(decl, args) -> Result<(), Unsatisfied>`
  in §3.1 and `more_constrained(a, b)` in §4.1 step 6; note "candidate
  template ignored: constraints not satisfied" + satisfaction detail.
- **WP-075** needs no hook (closures are classes, §2.4); **CTAD**: §2.8.

### 10.5 Landed (WP-068a): how the core is built
Where the code differs from (or details) the design above:
- **Callees.** An unqualified, unparenthesized name of a namespace-scope
  function that uses ADL, or any name denoting more than one function, is
  an overload set (`overload_name_ref`, `call.rs`). One candidate found by
  such a name still gets overload wording ("no matching function" plus the
  candidate note, `OverloadScratch::adl_single`); a qualified,
  parenthesized or block-scope name, and a member, take clang's direct
  path ("cannot initialize a parameter of type …" + "passing argument to
  parameter here"). `(g)(1)` keeps its `ParenExpr` (`try_member_call`
  likewise).
- **Newest-first order** (§13.1.1) is applied in `overload_name_ref`, which
  reverses unqualified function results, not in `lookup_name`: no other
  lookup consumer sees a changed order.
- **Member calls (§2.3).** `member_access` of an overloaded member builds a
  `MemberExpr` of type `QualType::OVERLOAD` (kiln's
  `UnresolvedMemberExpr`) and records the set in `OverloadScratch::members`
  (`OvlMember`), consumed by `resolve_member_call`; implicit `this->f`
  likewise (`implicit_member_overload`). A static winner named through an
  implicit `this` is a `DeclRefExpr`, as in clang.
- **User-defined conversions** (`udc.rs`): `apply_udc` builds the
  `before` sequence, the `CXXConstructExpr` or `CXXMemberCallExpr` of the
  conversion function (`conversion_function_call`), and the `after`
  sequence. A deleted conversion function reports
  `sema_conv_deleted_ctor` naming the initialized type (the non-reference
  target, cv kept). clang's CWG2327 workaround is kept: in an
  initialization by constructor, a conversion function yielding the class
  itself initializes the object directly (`best.rs`, `construct.rs`).
- **`IcsOptions::CANDIDATE_ARG`**: an argument of a candidate. In a
  reference binding, a deleted best [over.match.ref] conversion is no
  conversion (clang `FindConversionForRefInit`), so the candidate is not
  viable; a declaration's initialization still diagnoses the deleted
  function.
- **[over.match.list].** Phase 1 runs only when an initializer-list
  constructor is viable for the arguments (`init_list_phase1_viable`; an
  empty list with a default constructor value-initializes); otherwise
  phase 2 over all constructors. Building the `std::initializer_list`
  object itself is still unsupported.
- **Casts.** `static_cast` / functional / C-style casts from a class to a
  non-class type run the direct-initialization's user-defined conversion
  (explicit conversion functions are candidates) with clang's
  `tryDiagnoseOverloadedCast` errors (`sema_ovl_*_conversion_in_cast`).
- **Accounting (§1.4).** `Sema::overload_stats()` returns the `ovl.*`
  counters (`OverloadStats`); `Sema::overload_memory()` the
  `sema.ovl.pool` row. There is no `Sema::memory()` yet, and the driver
  does not print either, so `cargo xtask bench` records the `overload`
  group's wall time and peak RSS but not `ovl.*` (an open item: a report
  flag like `-fkiln-instantiation-report`). `sema.ovl.conv_types` and
  `ovl.builtin_candidates` are WP-068b's (§10.6).
- **Bench** `bench/overload/`: `calls` (200 overloads × 3,000 calls) and
  `ctors` (64 converting constructors, 5,440 copy/direct/list
  initializations). WP-068b adds `arrow-chain` (a 50-deep `->` chain,
  12,000 accesses) and `less-10k` (10,000 class `a < b` with ADL friends,
  100 unrelated visible overloads and built-in candidates).
- **Stubs.** `CandidateFailure::BadDeduction` (WP-071),
  `ConstraintsNotSatisfied` (WP-073) and
  `SetKind::Address` (WP-071: [over.over] of templates; non-template
  targets are resolved without a set in `address.rs`) are declared and
  unused.

### 10.6 Landed (WP-068b): operators
Where the code differs from (or details) the design above:
- **Dispatch.** `act_on_binary`/`act_on_unary`/`act_on_subscript_args`
  (multi-argument `[]`), call-on-object in `act_on_call`, and braced
  assignment right operands call `oper.rs::try_overloaded_operator` when
  an operand has class or enumeration type; it returns the built call, a
  converted built-in form (`OperResult::Converted`, the operands after the
  selected built-in candidate's user-defined conversions) or nothing (the
  built-in path). Non-member candidates: unqualified lookup plus ADL,
  with the enum-only filter of [over.match.oper]/3.2; members from the
  left operand's class; rewritten and reversed candidates per §5 with
  P2468 (`allows_reversed`, `rewritten_of`). The AST is
  `CXXOperatorCallExpr` (callee `operator_callee`: decay cast of a
  `DeclRefExpr`, found decl kept) and `CXXRewrittenBinaryOperator` whose
  only child is the semantic form. A static `operator()`/`operator[]`
  keeps the object operand, materialized when it is a class prvalue.
- **Built-in candidates (`builtin.rs`).** Built in clang's
  `AddBuiltinOperatorCandidates` order; the class operands' type sets
  (from their conversion functions, `sema.ovl.conv_types`) are cached per
  class, and one resolution memoizes each (argument, parameter, options)
  ICS. `ovl.builtin_candidates` counts the candidates added. The
  /3.3.4 duplicate drop (a built-in with the same parameter types as a
  non-member candidate) is the enum-pair rule. The assignment candidates'
  left operand uses `SUPPRESS_UDC | NO_TEMPORARY`. Notes print
  `built-in candidate operator<(int, int)`; the relational ambiguity
  golden has 144 notes on x86_64 and 121 on aarch64
  (`tests/conformance/over.built/relational-ambiguity-*.cpp`). A selected
  built-in candidate applies only the user-defined-conversion part of each
  operand's ICS (the built-in operator then converts as usual).
- **`->` (`arrow.rs`).** `arrow_base` follows `operator->` until a
  pointer: cycle detection by class (`sema_circular_arrow`) and a depth
  limit of 256 (`sema_operator_arrow_depth_exceeded` with clang's note
  backtrace and suppression). There is no `-foperator-arrow-depth`
  option; the note naming it is printed as clang does.
- **Surrogates (`surrogate.rs`).** One surrogate per conversion function
  to pointer/reference to function ([over.call.object]/2), noted
  `conversion candidate of type …`. Divergence: the object's
  `UserDefinedConversion` cast stays an lvalue (clang: prvalue).
- **`operator new`/`delete` (`alloc.rs`, wired into `new_delete.rs`).**
  Class scope first (bases included) unless `::new`/`::delete` or the
  type is incomplete, else global. The usual deallocation function
  prefers the alignment-matching one; in class scope the unsized one,
  globally the sized one for a complete type. Nothing suitable in a class
  is `sema_no_suitable_delete_member` plus `note_member_declared_named`
  per function found; a deleted class `operator new` is `sema_call_deleted_function`. A member
  `operator delete` without an exception specification is implicitly
  `noexcept` ([except.spec]/9).
- **Gaps (owners).** Reversed and rewritten operator function template
  candidates are not formed (see §10.7). Builtin `<=>` on scalars reports "not supported yet" where
  clang reports the missing `std::strong_ordering` (WP-082). No access
  check on class `operator new`/`delete`; no destroying
  `operator delete` (WP-061 follow-up). `x.operator+=(2)` (member access
  naming an operator-function-id) is a parse error: `act_on_member` and
  `member_access` take a `Symbol`, not a `DeclName` (parser follow-up).
  No `note_equality_comparison_to_assign` ("use '=' to turn this equality
  comparison into an assignment") anywhere in kiln (sema follow-up).
  `aka` of a template specialization with an expression argument prints
  `'T<0 + 1>' (aka 'T<1>')` where clang prints no `aka` (types/print).
  `ovl.*` counters are not printed by the driver (§10.5).

### 10.7 Landed (WP-071): template candidates
- `add_template_candidate` (`call.rs`) adds the specialization
  `deduce_call` forms as an ordinary candidate with
  `Candidate::template` set. When deduction fails, it adds a non-viable
  `BadDeduction` candidate whose note `deduction_note` writes. When the
  arity is wrong, the pattern is added so the arity note is the one
  printed.
- Callers:
  - `resolve_unresolved_call` (named calls; `f<args>` keeps only the
    templates);
  - `resolve_member_call` (`OvlMember::targs` holds `e.f<args>`);
  - `add_nonmember_oper` and `add_member_opers`, for non-reversed,
    non-rewritten operator candidates only. A reversed or rewritten
    template candidate still sets `skipped_templates`, and the
    operator's resolution then reports "not yet implemented".
- /2.5 (more specialized template): `order_template_candidates` runs
  `more_specialized` (spec/sema-templates.md §6, `templ/order.rs`) over
  each pair of viable template candidates before `best_viable`. It
  records the ordered pairs in `OverloadCandidateSet::tmpl_order`, and
  `better()` reads them, because it cannot deduce through `&self`.
- Candidate notes print `candidate function [with T = int]` and
  `candidate function template not viable` (CWG1391) as clang's `FnDesc`
  does (`diag.rs::spec_desc`).
- Not yet:
  - `ics.rs` (conversion function templates);
  - `arrow.rs` (`operator->` templates);
  - a diagnostic for `&f` when no template matches the target type. That
    case still reports "not yet implemented", where clang prints
    `address of overloaded function` with deduction notes.
- Address of a function template ([temp.deduct.funcaddr]):
  - `resolve_single_template_id` resolves `&f<int>` when exactly one
    template accepts the explicit args.
  - When no non-template matches the target, `address_template_candidates`
    deduces P (the template's function type) from A (the target type)
    through `deduce_spec(.., target)`. It keeps the specializations that
    match exactly, then the one more specialized than all the others.

## 11. Agent parallelization (WP-068 split)
WP-068 has more than five deliverables, so it splits. 068a merges §10.1–10.3
types first (interface PR, docs/AGENTS.md); 068b starts from that.

| WP | owns (files) | deliverables | acceptance |
|---|---|---|---|
| **068a** core | `overload/{mod,ics,best,diag,adl,call}.rs`; edits to `conv/{scs,mod}.rs`, `expr.rs` (ULE/UME, `act_on_call`), `lookup.rs` (order, buffer), `init.rs`; non-operator ids of §9.1 | named and member calls with ADL; ICS (std/ref/user/ellipsis/list); §4; ctor/copy/conv/ref/list init (§2.6–2.7); default args (§7); §8 call/ctor/conversion shapes; §9.2; counters | conformance `over.match.{funcs,call,viable,best,ctor,copy,conv,ref,list}`, `over.ics.{user,ref,list,rank}`, `over.over`, `basic.lookup.argdep`, `dcl.fct.default`, `class.conv.fct`; ast goldens `ast/over.*`; `conv_ics.rs` cases for /3.2.3–3.2.6; overload bench recorded |
| **068b** operators | `overload/{oper,builtin,arrow,alloc}.rs`; `expr.rs` one dispatch call per binary/unary/subscript site; `decl.rs` `check_operator_decl`; operator ids | §2.4–2.5, §5, §6, surrogates, memoized built-in tables | `over.match.oper`, `over.built`, `over.call.object`, `over.oper`, `over.ref`, `class.compare` (rewrites), `expr.new` (lookup); 144/121-note relational ambiguity per target; `__float128` per target |

Shared `expr.rs` edits are disjoint (068b: one dispatch call per site), as
are the diag-id blocks. **Bench** `bench/overload/` (mirrorbind-style): 200
overloads × 1000 calls, a 50-deep `->` chain, 10k class `a < b`; records
`ovl.*`, wall time, peak memory (spec/bench.md gates).

## 12. Tests
Directory = section label. `compile-fail` tests use `strict-diags` with
full note lists (note order checked); `ast` goldens are verified with
`cargo xtask clang-ast`; ranking rules are `compile-ok` tests that
`static_assert` on the selected overload's return type.

| `tests/conformance/…` | kind | covers |
|---|---|---|
| `over.match.funcs/` | fail, ast | object parameter cv/ref-qual, static members, using-declared members |
| `over.match.call/` | fail, ast | ULE vs direct wording (`g(1)`, `(g)(1)`, `N::f(1)`), member calls, missing object |
| `over.call.object/` | fail, ast | `operator()`, surrogates, no call operator |
| `over.match.oper/` | fail, ast | member/non-member/built-in mix, enum-only fallback, `=` restrictions, `adl` flag, rewritten/reversed, P2468, `-Wambiguous-reversed-operator` |
| `over.built/` | fail | built-in notes in order; all-pairs relational per target |
| `over.match.{ctor,copy,conv,ref,list}/` | fail, ast | explicit rules, list phases, [over.best.ics]/4 |
| `over.match.viable/` | fail | arity notes and order, defaults, ellipsis |
| `over.ics.{user,ref,list,rank}/` | ok, fail | one distinguishing pair per rule |
| `over.match.best/` | ok, fail | /2.1–2.9, ambiguity notes, deleted best |
| `over.over/`, `over.oper/`, `over.ref/` | fail, ast | targets, mismatch notes, unresolvable; declaration checks; `->` chain, cycle, depth |
| `dcl.fct.default/`, `basic.lookup.argdep/`, `class.conv.fct/`, `expr.new/` | ok, fail, ast | defaults and `CXXDefaultArgExpr`; ADL entities, hidden friends, suppression; explicit/contextual conversions; class `operator new` |

Unit tests (`kiln_sema/tests/overload_*.rs`): `compare_ics` pairs,
`compare_for_display`, `Ics` size, pool reuse (`MemReport` flat after warm-up).

## 13. Open questions (with recommendations)
1. **Note order for unqualified lookup** is clang's newest-first. Recommend
   reversing in `lookup_name` (§2.1), not changing ast.md storage order.
2. **`-Wambiguous-reversed-operator`**: recommend keeping clang's extension
   (§5.3); real code relies on it.
3. **`StandardConversion` growth**: recommend adding `rb` and
   `SecondStep::DerivedToBase` there (sema-conv.md §3), so [over.ics.rank]
   lives in one function.
4. **`MaterializeTemporaryExpr`/`ExprWithCleanups` ownership**: recommend
   WP-067 owns them and `perform_ics` calls its `materialize_temporary`; if
   068a lands first it carries a minimal version 067 replaces.
5. **`-fshow-overloads`**: recommend rejecting `best` as unsupported.
6. **Built-in candidate volume**: recommend generating all of clang's
   candidates (note fidelity) with the §1.4 caches; prune only if the bench
   says so.
7. **Hidden friends**: recommend S3C stores them in the namespace table with
   `LookupFlags::ADL_ONLY` rather than a side table.
8. **C++23 static `operator()` shape**: take clang 19's dump (068b golden).
9. **Direct-path wording** (`(g)(1)`, `N::f(1)`): recommend fixing in 068a
   with the ULE change and regenerating any M2 test that captured the
   overload wording.
10. **068 ordering vs 067**: 068a needs class temporaries for by-value class
    arguments; recommend 068a after 067, or 068a scoped to non-class
    arguments first.

### 13.1 Supervisor decisions (2026-10-28, binding)
1. **Accepted.** `lookup_name` yields unqualified-lookup results newest-first
   along the scope chain; ast.md §4.3 storage order is unchanged.
2. **Accepted.** Keep the reversed-`==` ambiguity extension with
   `-Wambiguous-reversed-operator`.
3. **Accepted.** `StandardConversion` gains the reference-binding record and
   `SecondStep::DerivedToBase`; sema-conv.md §3/§4 are amended by WP-068a,
   which also fixes the /3.2.5-vs-/4.x ordering (§13.2.1) to clang's order.
4. **WP-067 owns** `MaterializeTemporaryExpr`/`ExprWithCleanups`; `perform_ics`
   calls its `materialize_temporary`. No interim version: see 10.
5. **Accepted.** `-fshow-overloads=best` is rejected as unsupported; all
   candidates are always shown.
6. **Accepted.** Generate all of clang's built-in candidates; the §1.4 caches
   are obligatory, and `bench/overload` gates them.
7. **Accepted and binding on S3C:** hidden friends live in the namespace
   lookup table with `LookupFlags::ADL_ONLY`; no side table.
8. **Accepted.** C++23 static `operator()` takes clang 19's dump shape via a
   068b golden.
9. **Accepted.** The direct-call wording is fixed in 068a; M2 goldens that
   captured the overload wording are regenerated from clang.
10. **068a runs after 067** (needs class temporaries); 068b after 068a. The
    BACKLOG rows are split accordingly. ADL (`adl.rs`) is part of 068a and
    sema.md §3 is amended by it (§13.2.4).

### 13.2 Inconsistencies found in existing specs and code
1. sema-conv.md §4 (l. 95–101) orders 3.2.5 before 4.1–4.4;
   `conv/scs.rs` (l. 640–700) checks it after 4.4; clang differs from both (§3.4).
2. `StandardConversion` (`conv/scs.rs` l. 93) has no reference-binding data:
   [over.ics.rank]/3.2.3, /3.2.4, /3.2.6 are inexpressible (§3.3).
3. `expr.rs` `act_on_call`/`bad_argument` (l. 1686–1730) use overload wording
   for `(g)(x)` and single `N::f(x)`; clang says "cannot initialize a
   parameter …" + "passing argument to parameter here" (§2.1).
4. sema.md §3 (l. 59–79) has no ADL; ast.md §4.3 (l. 200) assumes it.
5. ast.md §4.3 (l. 190–192): overload sets in first-declaration order;
   clang's unqualified order is newest-first along the scope chain (§2.1).
6. `lookup.rs` `Found` (l. 32) allocates a Vec per lookup (§1.4);
   `conv/mod.rs` `has_constructors_or_unknown` (l. 676) is a stub.
