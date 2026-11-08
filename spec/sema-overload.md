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
  `sema.ovl.conv_types`, and (WP-661) `sema.redecl_index` (count = keyed
  positions) and `sema.explicit_memo` (count = slots in use, bytes = the
  table once allocated). Bench counters (spec/bench.md): `ovl.sets`,
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
order). A ULE naming a function of dependent type (a static member
`key(const node<K>*)` of a class template beside `key(const nb*)`) is
type-dependent ([temp.dep.expr]/3, clang's `OverloadExpr` of decls in a
dependent context): its call is resolved at instantiation, not at the
definition, where the dependent parameter's conversion is unknown
(libstdc++ `_Rb_tree::_S_key`; test
`temp.dep.expr/static-member-overload-dependent-type.cpp`). So is a ULE
naming any member of a dependent class, member templates included
(clang's rule: a decl whose context is dependent), even when the call's
arguments are not dependent: SFINAE-selected static member templates
`to_char<C = T>(unsigned char)` of a class template are resolved per
instantiation, not reported ambiguous at the definition (WP-668; test
`temp.dep.expr/member-template-overload-dependent-context.cpp`).

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
`UseArgumentDependentLookup`) for qualified or parenthesized callees
(a parenthesized name of a function template too: its
`UnresolvedLookupExpr` is built without ADL, so `(f)(t)` in a template
does not find an associated namespace's `f` at instantiation; boost's
`(iterator_arrow_result)(temp)`, WP-726,
`tests/conformance/basic.lookup.argdep/parenthesized-function-template-no-adl.cpp`),
implicit builtins, or when ordinary lookup found a class member, a
block-scope function declaration (not a using-declaration), or anything
that is not a function or function template. The suppression holds for a
call made dependent by its arguments too: `dependent_call` keeps an implicit
builtin callee as its `DeclRefExpr` instead of an ADL `UnresolvedLookupExpr`,
so instantiating `__builtin_addressof(x)` with `x` a `Holder<Incomplete>*`
does not instantiate `Holder<Incomplete>` ([temp.inst]/2; WP-310,
`temp.inst/builtin-call-no-adl-completion.cpp`).

**Associated entities** (/3), `associated_entities(ty, &mut AssocSet)`:
pointers/references/arrays → element; function types → parameters and
return; member pointers → class and pointee; enum → innermost enclosing
namespace (+ class if a member); class → itself, all bases, their innermost
enclosing namespaces; a template specialization adds its own template
arguments' entities (not a base's, nor an enclosing class's: WP-418,
`tests/conformance/basic.lookup.argdep/base-template-args-not-associated.cpp`,
libc++ `iterator_robust_against_adl`); an overload-set (ULE) argument adds every candidate's;
`std::meta::info` → the enumeration `std::meta::operators` and namespace
`std::meta` once `<meta>` declared it (/3.1, P2996R13; the namespace is
cached in `Sema::std_meta_ns`), so unqualified `type_of(r)` finds
`std::meta::type_of` (WP-796 via WP-879,
`basic.lookup.argdep/p3-meta-info-std-meta*.cpp`, compared with GCC 16.1).
An associated class that is an undefined class template specialization (or
member class of one) is instantiated first, silently, from the call's
location, and the set recomputed until nothing new is instantiated (clang
`addAssociatedClassesAndNamespaces` calls `isCompleteType`): its hidden
friends and bases then take part ([temp.inst]/3; WP-223,
`basic.lookup.argdep/instantiates-associated-class.cpp`).
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

A template-id naming member function templates and called without an
object expression (`get<I>()` found in a class, `B::get<I>()` qualified,
also inside `B`) is an `UnresolvedLookupExpr` until the call
(`resolve_unresolved_call`); there, where `this` may be used and its class
is (derived from) the members' class ([class.mfct.non.static]/2), it becomes
the UME `(*this).get<I>` with the explicit arguments
(`implicit_member_template_id`), so the implicit object parameters are
ranked against `*this` exactly as for `this->get<I>()` and for a
non-template name (const `*this` picks the const overload, non-const the
non-const one). A type-dependent `this`, or a dependent base on the way to
the members' class, gives the dependent `this->get<I>` resolved at
instantiation. The naming class is the qualifier's class, else the class of
`*this`. A call whose candidates all need an object (non-static, no
explicit object parameter) where no such `this` exists (a static member
function, an unrelated class) is "call to non-static member function
without an object argument" at the name before overload resolution, with
or without template arguments, also in an unevaluated operand (clang's
`IMA_Error_StaticContext` / `IMA_Error_Unrelated`; WP-543).

A qualified `F::f(...)` in a non-static member of a class not derived from
`F`, naming a set that mixes static and non-static members, is still an
implicit member access (clang keeps the `UnresolvedMemberExpr` on
`(*this)`, `IMA_Mixed_Unrelated`): overload resolution runs against
`*this`, so a static winner is called and a non-static winner is "no
matching member function for call to 'f'" with each member's candidate
note (an object-argument mismatch is "no known conversion from 'B' to 'F'
for object argument"). The object conversion of a member template
candidate is checked before its specialization is formed (clang
`CheckNonDependentConversions`), so its failure notes the template
("candidate function template not viable"), not `[with U = int]`.
WP-788 (WP-823); test
`class.mfct.non.static/qualified-mixed-set-from-unrelated-class.cpp`.

#### Member overload sets used other than called
A class member access naming an overload set (`g.f`, `g.Ns<1>`) that is
not called follows clang's member `OverloadExpr` rules
(`Sema::diagnose_member_set_not_called_at`,
`single_member_template_spec`):
- every member is a non-static member function (a bound member set):
  "reference to non-static member function must be called", with the
  suffix "; did you mean to call it with no arguments?" when at least one
  candidate takes no argument and all such candidates return the same type,
  and a "possible target for call" note per member. This holds for every
  use: an initializer, a cast, a placeholder, a discarded `g.f;`. It is
  reported at the outermost parenthesis of `(g.o)`. A placeholder reports
  it twice, as clang does, and with the hint the expression is recovered
  as the call (spec/sema.md §1.2, WP-832). A type-dependent access (the
  current instantiation's members in a template body: `this->f`,
  `F::tm<long>`) gets no hint, since clang's `tryExprAsCall` tries no call
  on it; a member template-id resolved early with an implicit object
  still lists the template as the possible target (WP-888).
- a template-id naming exactly one specialization deducible from its
  explicit arguments alone, static: it is that function lvalue, so a
  placeholder deduces from it (`auto` a pointer, `const auto` a const
  pointer, `auto&` and `decltype(auto)` a reference) and `&g.B<1>` is a
  function pointer ([dcl.spec.auto.general]/4, [over.over]/3; clang
  `resolveAndFixSingleFunctionTemplateSpecialization`);
- anything else (several candidates, a static member among them): a
  placeholder is "variable 'x' with type 'auto' has incompatible
  initializer of type '<overloaded function type>'".
WP-788 (WP-823, WP-888); tests
`dcl.spec.auto/member-template-id-in-template-body.cpp`,
`dcl.spec.auto/member-template-id-single-static-specialization-run.cpp`,
`dcl.spec.auto/member-set-placeholder-and-unevaluated-use.cpp`.

### 2.4 Call to an object of class type ([over.call.object])
`resolve_call_object(obj, args)`. Candidates: every `operator()` of `T`
(member lookup), then one **surrogate** per non-hidden, non-explicit
conversion function of `T` (and bases) to pointer/reference to function
whose cv-qualification is ≥ the object's: parameters `(conversion's object,
P1..Pn)`, `CandidateKind::Surrogate { conv }`, note "conversion candidate of
type 'void (*)(int)'". Neither present: "type 'H' does not provide a call
operator". Failure: "no matching function for call to object of type 'F'" /
"call to object of type 'F' is ambiguous".
Only `T` is completed up front: there is no argument-dependent lookup, so
an argument's class template specialization is instantiated only when a
viable candidate's conversion needs it (a candidate rejected by its
constraints converts nothing; clang `BuildCallToObjectOfClassType`).
WP-418, `tests/conformance/over.call.object/argument-class-not-completed.cpp`
(libc++ `ranges_replace_copy_if`).

### 2.5 Operators ([over.match.oper])
`resolve_operator(op, args, loc)`:
1. No operand of class or enum type (after reference stripping) → `None`,
   the M2 built-in path runs. `= [] () ->` resolve only for a class left
   operand.
2. **Member candidates**: qualified lookup of `operator@` in `T1` if complete
   or being defined (also for `()`, [over.call.object], and for each
   reversed/rewritten form). An ambiguous lookup ([class.member.lookup]/6)
   is diagnosed at the operator ("member 'operator()' found in multiple
   base classes of different types", as clang's `~LookupResult`), and every
   declaration found stays a candidate (WP-152). An undefined class
   template specialization is instantiated if it can be; an operand of a
   class that stays incomplete (the template is only declared, libc++
   `ostream&` from `<iosfwd>`) contributes no member candidates and is
   not diagnosed (clang `isCompleteType`; `require_complete_silent`,
   WP-201).
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
| [over.match.ref] reference to `cv2 T2` | `resolve_conversion(T, e, ToReference{lvalue})` | conversion functions yielding lvalue `cv2 T2&` (lvalue ref) or rvalue `cv2 T2`/`T2&&`, reference-compatible; explicit ones only in direct-init (`IcsOptions::EXPLICIT_REF_CONV`) |
| [over.match.list] list-init of class `T` | `resolve_ctor(T, list, List)` | phase 1: initializer-list ctors with the list as sole argument; phase 2 (none viable): all ctors with the elements |

The converting constructors of `T` include its inherited ones
([namespace.udecl]/13) wherever they are collected
(`Sema::converting_ctors`): the user-defined conversion of an implicit
conversion sequence (`build_conversion_set`, so `d = 3` reaches the
implicit copy assignment of `D` through an inherited `B(int)`) as well as
a copy-initialization (WP-686).

**`explicit`** (clang behaviour): in copy-init, explicit constructors and
conversion functions are added but fail with `CandidateFailure::Explicit`;
"explicit constructor is not a candidate" / "explicit conversion function is
not a candidate" notes appear only under "no viable conversion from …",
never in call-argument notes (there `tg(1)` with `explicit G(int)` just says
"no known conversion from 'int' to 'G' for 1st argument"). In
copy-list-init explicit ctors stay viable and choosing one is "chosen
constructor is explicit in copy-initialization" + "explicit constructor
declared here".

A conversion function template is deduced against the target type of
each conversion ([temp.deduct.conv]) from its own template parameter
list, also when the conversion is formed again inside an instantiation
of an enclosing function or class template (or a lambda in one): the
instantiation strips the definition's implicit conversion call to its
object and redoes the conversion (spec/sema-templates.md §5.4), so the
enclosing instantiation's arguments never stand in for the member
template's (WP-563).

A conversion function (or conversion function template specialization)
whose trailing requires-clause is not satisfied is not viable
([over.match.viable]/3), checked after the object argument as clang's
`AddConversionCandidate` does. So `operator int() requires false` on a
non-const object does not hide a satisfied `operator int() const`, and
"no viable conversion" notes it as "candidate function not viable:
constraints not satisfied" with the satisfaction notes (WP-302).
A surrogate call function ([over.call.object]/2) is not viable either
when its conversion function's constraints are not satisfied
(`add_surrogate`, clang `AddSurrogateCandidate`), and no surrogate note
is printed for it (WP-881).

**Reference direct-initialization** (`int&& r(Ex{})`, `static_cast<T&&>`;
clang `allowExplicitConversionFunctionsInRefBinding`): `bind_reference`
passes `IcsOptions::EXPLICIT_REF_CONV` when its `ConvCtx` is direct. The
[over.match.ref] sets then admit explicit conversion functions (as
`ALLOW_EXPLICIT`), and so does a non-class /5.4 temporary's
[over.match.conv] set (a prvalue `int` from `explicit operator int()` binds
`int&&`/`const int&`, subject to the same-type filter, so `const long&`
from it fails). A class temporary's constructors stay non-explicit-only
([dcl.init.ref]/5.4.1 copy-initializes it). Copy-initialization keeps "no
viable conversion" + "explicit conversion function is not a candidate".
The same-type filter (clang `isAllowableExplicitConversion`) applies to the
[over.match.ref] sets too whenever explicit functions are admitted: an
explicit conversion function yielding a derived class (`explicit operator
D()`, `explicit operator D&()`) binds no `const B&`/`B&&` and is dropped
without a note (tests `over.match.ref/explicit-direct-same-type-only.cpp`).

**Copy or move constructor of a direct-initialization** ([over.match.copy]/1.2,
[over.match.ref]/1.1; clang `ResolveConstructorOverload` `AllowExplicitConv`,
`hasCopyOrMoveCtorParam`): `resolve_ctor_ex` with one argument and a direct
style (`Direct`, `DirectList`) passes `IcsOptions::DIRECT_COPY_CTOR`, which
`add_found_candidates` turns into `EXPLICIT_REF_CONV` for each non-template,
non-inherited constructor whose first parameter is (a reference to) cv `T`
(`copy_or_move_ctor_param`; as clang, not for a constructor template). So
`R r(G{})` with `explicit operator R&()` (or `template <class T> explicit
operator T&()`) copies through `R(const R&)`, and so do `R(G{})`, `R{G{}}`,
`static_cast<R>(G{})`, `new R(G{})` and a mem-initializer. The selected
constructor's argument is then converted with `ConvCtx::DirectInitialization`
(`CtorCall::direct` → `ctor_args(.., explicit_first)`), the same options the
sequence was formed with. Tests
`over.match.copy/direct-init-explicit-conversion-to-copy-ctor.cpp`.

When the /5.4.1 source is not a class (`const B& b = 1`), clang's
reference-init texts apply instead (WP-161): no usable constructor is
"reference to type 'const B' could not bind to an rvalue of type 'int'"
(`sema_reference_bind_failed`), an ambiguous one is "conversion from
'int' to 'const F' is ambiguous" at the initializer, and a deleted one is
"attempt to use a deleted function" (`DeletedAs::Use`).
A `return` operand copy-initializing a class with no viable converting
constructor is clang's "no viable conversion from returned value of type
'int' to function return type 'S'" (`sema_no_viable_conversion_return`)
whatever the operand's type, a non-class or `_Atomic(T)` one included
(`construct_converted` passes `DeletedAs::Return`; WP-183).

### 2.7 [over.best.ics]/4 restrictions
`IcsOptions::SUPPRESS_UDC`, passed by the nested resolution, forbids
user-defined conversions for the argument of a constructor's first
parameter (of class `X`) when the context is the second step of a class
copy-init, [over.match.copy]/[conv]/[ref], or [over.match.list] phase 2 with
a single initializer-list argument converting to `X` or `cv X&`. This also
breaks copy-ctor ↔ converting-ctor recursion.

The [over.match.list] phase 2 case (WP-373, clang `IsUserDefinedConversion`
and `ResolveConstructorOverload`) applies only when the single element is
itself a braced-init-list: `IcsOptions::LIST_COPY_CTOR` is set by the list
conversion (`list_ctor_conversion`) and by `resolve_ctor_ex` for a single
braced argument of a list-initialization, and becomes `SUPPRESS_UDC` only
for a copy or move constructor's first parameter. A list converting to a
non-aggregate class under `SUPPRESS_UDC` is a bad ICS ([over.ics.list]/7
needs a user-defined conversion) unless its single element is of that class
or derived from it ([over.ics.list]/2). So `h({{1}})` with `H(E)` picks
`H(E)` (the copy/move constructors are not viable for `{1}`) and reports
the explicit `E(int)` chosen in copy-list-initialization, while `H
hh({1})` (element `1`, not a list) leaves `H(E)` and both copy/move
constructors viable and is ambiguous. A braced argument of a constructor
call is list-initialized with the constructor parameter's "passing argument
to parameter here" note (`define.rs` `ctor_args`). Tests:
`over.best.ics/list-copy-ctor-ambiguous.cpp`,
`over.best.ics/nested-braced-explicit-ctor.cpp`.

### 2.8 CTAD hook
S3T builds the [over.match.class.deduct] guide set and resolves it with
`SetKind::Guides`/`CandidateKind::DeductionGuide` (§4.1 step 10); M3 only
reserves these.

### 2.9 Address of an overloaded function ([over.over])
`resolve_address_of_overloaded(ule, target, complain) -> Option<ExprId>`.
Targets: an initialized object or reference, assignment LHS, a parameter
(during ICS a ULE argument converts by [over.over]), a cast, a non-type
template parameter, a return. A class type, or a reference to one, is not
a target: when a converting constructor takes the set (`dynamic(ObjectMaker
(*)())`), the set is that constructor's argument
(`overload_set_converts_to_class`, so `const dynamic& v = dynamic::object`
binds a temporary); otherwise the set is resolved against the class type
and finds no match, as in clang. This is the general [over.match.copy] /
[over.match.ctor] rule, not one shaped to a single constructor (WP-779,
`over.over/class-target-general.cpp`, `class-target-ambiguous-or-none.cpp`):
each constructor's parameter selects from the set on its own, so copy-,
direct- and list-initialization, a return, a by-value argument, `&f` and
`&S::m` sets all convert; two constructors that both accept are ambiguous,
a constructor template cannot deduce from a multi-function set
([temp.deduct.call]/6), and an `explicit` one is no copy-init candidate. A non-template matches when its type equals
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

An unqualified member name inside a member function that names a set of
static and non-static member functions is an implicit member overload set
(`this->f`, §2.3; also the `this->f` an instantiation rebuilds it as, whose
`this` is implicit). With a pointer-to-function target, of `f` or `&f`, the
static members are its candidates (`implicit_member_operand`, WP-402): a
matching non-template, else ([over.over]/3, /5, WP-416) the specialization
of a static member function template deduced from the target, the most
specialized of several (`address_template_candidates` over the set). The
one selected becomes a `DeclRefExpr` and the implicit object is dropped
(clang's `FixOverloadedFunctionReference`); a non-static member, template
or not, is never named this way ([expr.prim.id.general]/4,
[expr.unary.op]/4). `&f` of any such set is a lookup of the name (clang's
`ActOnIdExpression` builds a ULE for an overloaded address-of operand), so
with no match it is "address of overloaded function 'f' does not match
required type" with the ULE's candidate notes (a mismatch note per
non-template, deduction failures after), and so is `f` when the set has a
static member or static member template (clang's `UnresolvedMemberExpr` is
then of overloaded type); `f` of only non-static members, or with a
member-pointer target, is "reference to non-static member function must be
called" with the candidates as "possible target for call", and `&(f)` is
"cannot create a non-constant pointer to member function". The
"does not match" notes follow clang's lookup order: inside a member
function defined in its class the class scope is still open and finds the
members newest first (`in_inline_member_body` reverses the set), out of
line the class's lookup table gives declaration order. Tests:
`over.over/implicit-member-template-address.cpp`,
`over.over/implicit-member-note-order.cpp`,
`tests/codegen/implicit_member_template_address.cpp`, and
`over.over/implicit-member-static-target.cpp` (libc++ 22 `<format>`
`__allocating_buffer` passing `__prepare_write` to its base).

A function template-id with no call and no target (`&f<2>`, `auto e =
f<6>`) is resolved at once (`resolve_single_template_id`, [over.over]/3 via
[temp.arg.explicit]/4), but clang keeps it an overload set until its use, so
a deleted specialization is not diagnosed at its name (WP-373): it is
pending (`defer_deleted_template_id`, qualified or not), widened by each
enclosing parenthesis (`act_on_paren`) and `&` (`act_on_unary`) to start at
their location, then reported at the end of the full-expression as "attempt
to use a deleted function" at that start (`&(f<3>)`, `(&f<5>)`, `&N::f<4>`
all at the first token), or by a call through it as "call to deleted
function 'f'" naming the template and noting "candidate function [with I =
7] has been explicitly deleted". Test:
`over.over/deleted-template-id-location.cpp`. Only the templates of the
set count (`single_template_spec`): a non-template function found by the
same lookup is skipped, as clang's template-name lookup drops it
(`FilterAcceptableTemplateNames`), so `int rm(const char *)` beside
`template<class T> void rm(T &, int)` leaves `&rm<M>` one specialization
(WP-690, `over.over/template-id-with-non-template.cpp`).

A called function template-id is not resolved so, parenthesized or not:
`(f<A>)(x)` and `((N::f<A>))(x)` are the overload set of `f<A>(x)`
([temp.arg.explicit]/4, [over.call.func]/3, without argument-dependent
lookup), so the call deduces the remaining template arguments, a trailing
parameter pack included, and the specialization with only the explicit
arguments is never formed or instantiated (the parser's
`template_id_is_callee`: a template-id followed by `(`, or the whole of a
parenthesized expression followed by `(`). The parentheses stay around the
resolved name in the AST, as for `(f)(x)` (clang's
`FixOverloadedFunctionReference`). Test:
`temp.arg.explicit/parenthesized-template-id-callee.cpp` (WP-695). The
same rule holds for an operator function template-id, `(operator+<A>)(x,
y)` and `(::operator+<A>)(x, y)` (`parse_operator_template_id`), and for a
splice template-id, `(template [: r :]<A>)(x, y)`
(`parse_splice_template_args`). WP-699 (WP-823); tests
`temp.arg.explicit/parenthesized-operator-template-id-callee.cpp`,
`temp.names/parenthesized-splice-template-id-callee.cpp`.

## 3. Viable functions ([over.match.viable])

Some functions are never added as candidates (`add_candidate`). These are
a defaulted move constructor or move assignment operator that is defined as
deleted, whether implicit or `= default` ([class.copy.ctor]/10,
[class.copy.assign]/7, CWG1402). Such a function is not listed in the notes
either. An rvalue argument then selects the copy, as in clang's
`AddOverloadCandidate` and `AddMethodCandidate`. An explicitly deleted move
member still participates.

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
X&&` (`&&`), `X` the naming class, `cv` from `FuncInfo::this_quals`; a
member found through a using-declaration (a `UsingShadow`) takes the
using-declaration's class as `X` ([over.match.funcs.general]/4, clang's
`ActingContext`; `object_ics_found`, WP-146). The
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
`ALLOW_EXPLICIT` (direct-init, contextual bool), `CANDIDATE_ARG`,
`EXPLICIT_REF_CONV` (reference direct-init, §2.6), `DIRECT_COPY_CTOR`
(the constructor set of a one-argument direct-initialization, consumed by
`add_found_candidates`, §2.6).

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
| aggregate class, one undesignated element of that class or a class derived from it | the element's ICS to the class ([over.ics.list]/3, CWG2076; WP-724, `tests/conformance/over.ics.list/aggregate-single-element-of-class-run.cpp`) |
| aggregate class | `UserDefined{via: Aggregate}` if aggregate init is well-formed. An undesignated list is walked like clang's verify-only aggregate initialization (`aggregate_list_walk`, WP-315). The walk covers each base and then each field (a union's first field; array elements one by one), consuming clauses with brace elision ([dcl.init.aggr]/16) as `init_element` decides it. A braced clause must have a viable list ICS to its element. An unbraced clause either starts an elided subaggregate (an array not initialized by a string literal; an aggregate class when the clause is not of that class or a derived class and does not convert to it) or must implicitly convert to the element. It is not viable when: clauses are left over; an elision reaches an aggregate with no elements (`aggregate_has_no_elements`, the rule `init_element` diagnoses, spec/sema.md §6); an element left over is a reference; or an element left over is a class with no viable `{}`. An element left over does not count if it has a default member initializer or is an anonymous member. Tests: `tests/conformance/over.ics.list/aggregate-brace-elision-viability*.cpp`. A designated list, or one the walk cannot decide (an incomplete or unbounded element, an error-typed clause, a GNU vector element), is bounded from above instead. It must designate no subobject twice and have no more elements than brace elision reaches (`aggregate_list_capacity`): each base and field takes one clause if it is a scalar or non-aggregate class, else its own capacity but at least one, so an empty class, an empty base or libc++'s `std::array<T, 0>` member takes `{}` (WP-304, `tests/conformance/over.ics.list/aggregate-elision-capacity*.cpp`) |
| reference, one undesignated element reference-related to the referee | the element's reference-binding ICS, also when it is Bad and with its own note ([dcl.init.list]/3.10, clang `TryListConversion`; WP-724, `tests/conformance/over.ics.list/aggregate-single-element-of-class-errors.cpp`) |
| reference | as the referenced type, bound to the temporary |
| non-class, one element / empty | element's ICS (narrowing → `Bad(Narrowing)`) / identity |

Wording: "cannot convert initializer list argument to 'int'". Every failure
of a braced argument is `Bad(InitList)` naming the parameter type (for a
reference, the cv-qualified referenced type: `'const G'` for the implicit
copy constructor), as clang's `TryListConversion` fails with the list as the
source (WP-152). A non-const (or volatile) lvalue reference whose temporary
would list-initialize is instead `Bad(LvalueRequired)` ("expects an lvalue
for 1st argument"); a single-element candidate call reports the same detail
(`forms.rs` `braced_argument`;
`tests/conformance/over.ics.list/reference-parameter-notes.cpp`).

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
10. /2.10–2.13 CTAD guides (§2.8): a guide not from an inherited
    constructor beats one that is, when the parameter types for the
    arguments are the same (P2582, spec/sema-templates.md §6.6.1); declared
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

### 4.3 `enable_if` overloads (clang extension; WP-460, WP-579)
Clang's `__attribute__((enable_if(cond, "msg")))` (spec/sema.md §4, the
`enable_if` bullet, owns parsing and viability) makes conditions part of a
function's identity, so a set may hold overloads that differ only in them
(abseil's `SpinLock(base_internal::SchedulingMode)` pair):
- Identity (clang `Sema::IsOverload`): two declarations with the same
  signature declare one function only when their condition lists are equal
  in count and order, each pair equivalent by [temp.over.link]/5 profiles
  (`Sema::same_enable_if`); the messages are ignored. Equal lists keep every
  existing redeclaration rule: a second definition is "redefinition of
  'f'", a second declaration of a constructor or member function in its
  class "constructor cannot be redeclared" / "class member cannot be
  redeclared". The list is compared on every declaration path:
  namespace-scope and out-of-line declarations (`NewFn::enable_if`, read by
  `fn_corresponds` / `pick_prev_function`), member functions in their class,
  special members (`act_on_special_declarator`,
  `class/special/declare.rs`), and instantiated members and member
  templates of a class template, whose conditions are the pattern's
  (`Sema::inst_enable_if`, set by `Substituter::function_decl` before the
  instantiated declaration is checked, since its attributes are copied
  only afterwards).
- Tiebreak (clang `compareEnableIfAttrs`, `Sema::compare_enable_if`): the
  last [over.match.best] criterion, after those of §4.1: a candidate with
  conditions beats one without; between two with conditions, compared in
  order, the one whose list properly extends the other's equivalent
  prefix wins; otherwise neither is better, an ambiguity ("call to 'f' is
  ambiguous" / "call to constructor of 'S' is ambiguous" with both
  candidates noted).
- `[[deprecated]]` and the rest of the post-selection checks of §4.2 apply
  to the selected overload only. A selected deprecated constructor warns at
  the variable for direct- and list-initialization, at the initializer for
  copy-initialization from a class prvalue, and at the converted expression
  for a converting constructor of a user-defined conversion (clang
  `PerformConstructorInitialization` / `DiagnoseUseOfDecl`;
  `Sema::warn_deprecated_use` from `use_ctor`, `CtorCall::udc`).
- Mangling (clang `mangleFunctionEncodingBareType`; `kiln_abi` mangle.rs
  `enable_if_conds`): `<name> Ua9enable_ifI <template-arg>* E
  <bare-function-type>`, each condition in declaration order as a template
  argument expression (a non-primary one in `X…E`) inside the function's
  prototype scope (a parameter is `fL0p_`, an enumerator its literal
  `L <enum-type> <value> E`). The message is not mangled. An unsupported
  condition falls back to `XLi0EE` (spec/abi.md §8.15), so two such
  overloads in one translation unit can still collide.

### 4.3a `pass_object_size` overloads (clang extension; WP-803)
A function with a `pass_object_size` parameter (spec/sema.md §4) never
redeclares one without it, in either order (clang `IsOverload`;
`NewFn::pass_object_size`, compared in `fn_corresponds_in`), and is not
merged with an `extern "C"` function of the same name. In
[over.match.best], after the `enable_if` tiebreak of §4.3, a candidate
with such a parameter beats one without (clang `isBetterOverloadCandidate`
`HasPassObjSize`). Such a function cannot have its address taken: it is
skipped by §2.9 address resolution (`address_matches`,
`single_overload_candidate`). A using-declaration whose target is one of
the two does not conflict with the other already in scope
([namespace.udecl]/11; `using_conflicts`; libstdc++'s `<stdlib.h>`
re-exports glibc's fortify wrappers that way; test
`namespace.udecl/using-pass-object-size-overload.cpp`). Mangling:
spec/abi.md §8.28.

### 4.4 Deferred template candidates (clang 21; `overload/defer.rs`)
At `-fclang-compat=22` most candidate sets resolve in two phases (clang
21's `TwoPhaseResolution`). `defer_templates(set)` turns it on (sets
`two_phase`), and a function template named without explicit template
arguments is then recorded as a `Deferred` entry rather than deduced
(`Template`, `NonmemberOper`, `MemberOper`, `Found`, `Guide`, each
holding what its adding context needs to add it later). `resolve_set`
replaces `order_template_candidates` + `best_viable`:
- If templates were deferred and `no_perfect` is clear, it looks for a
  perfect match among the candidates added so far (`perfect_viable`,
  clang `PerfectViableFunction`). A candidate is perfect when it is
  viable and every ICS, including the object argument's and a
  conversion function's final conversion, is an identity standard
  conversion (lvalue transformations aside) that is not from a braced
  list. A reference binding must be to the argument's own cv-qualified
  type, and an lvalue reference must not bind an rvalue
  (`perfect_conversion`). A static member function's object slot, and
  that of a member function named without an object, is never perfect.
  Of several perfect candidates, the one more constrained than every
  other wins. Otherwise (equally constrained, a built-in or surrogate
  among them), or for a perfect conversion function while constructor
  templates are deferred, there is no perfect match. A deleted perfect
  match injects the deferred templates (for the notes) and is `Deleted`.
- Otherwise `inject_deferred` deduces every deferred entry in order
  (with its original point of instantiation and ADL marking), then the
  usual ordering and selection run.

Sets that defer: calls, member calls, operators (including rewritten
and reversed forms), `->`, initialization by constructor
(`resolve_ctor_ex` for direct/list styles or copy from the same or a
derived class), special-member lookup (`deletion.rs`), range-`for`
ADL `begin`/`end`, and CTAD. Initialization by user-defined conversion
does not defer. `no_perfect` is set by a constructor template with a
dependent *explicit-specifier* (`dependently_explicit`, added at once)
and by a conversion function template in an elision set (clang
`DisableResolutionByPerfectCandidate`). At `-fclang-compat=19`
nothing defers. Not yet deferred: contextual conversion sets
(`switch_conv.rs`). Tests: `tests/conformance/over.match.best/perfect-match-*.cpp`,
`run-perfect-match-skips-templates.cpp`.

## 5. Rewritten comparison candidates ([over.match.oper]/3.4, P2468)

### 5.1 Generation (`oper.rs::add_rewritten`)
The §2.5 lookups are repeated for the names below; each hit adds a
candidate with `rewrite: Rewritten` (operands `x, y`) or `Reversed`
(operands `y, x`). Built-ins are never rewritten. Rewritten and
reversed candidates exist only from C++20 (clang `CreateOverloadedBinOp`
clears `AllowRewrittenCandidates` before it, WP-730): in C++17, `a != b`
never considers `operator==`, so a base's member `operator!=` beside a
more-derived base's friend `operator==` (LLVM's `iterator_facade_base`)
is not ambiguous.

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
- Reversed `x <=> y` ([over.match.oper]/8): `CXXRewrittenBinaryOperator` →
  `0 <=> (y <=> x)`, the nested `resolve_operator(<=>, [0, y <=> x])` as
  above (for `std::strong_ordering`, its `operator<=>(_CmpUnspecifiedParam,
  strong_ordering)`), never the bare `y <=> x` call
  (`tests/conformance/over.match.oper/reversed-spaceship-run.cpp`).

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
there is no `operator[]` and no built-in is viable. A type-dependent
`a[b...]` with one plain index stays an `ArraySubscriptExpr`; with several
indices or a pack expansion (`m[is...]`, [temp.variadic]/5) it is a
`CXXOperatorCallExpr '[]'` of the dependent type (prvalue) whose callee is
an empty ADL `UnresolvedLookupExpr` of `operator[]` (clang
`CreateOverloadedArraySubscriptExpr`); substitution expands the indices and
dispatches again (`act_on_subscript_args`; WP-292). Before C++23 the
index is one *expression* ([expr.sub]/1 of C++20): the parser parses `a[x, y]`
as one comma expression, so a class's `operator[]` receives one argument;
in C++20 a top-level comma (a built-in or an overloaded `operator,`) there
warns `-Wdeprecated-comma-subscript` "top-level comma expression in array
subscript is deprecated in C++20 and unsupported in C++23" at the outermost comma
([depr.comma.subscript]; WP-681, `tests/conformance/depr.comma.subscript/`).
Postfix `x++` calls with
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
// as built (overload/alloc.rs): AllocFns { fns, templates, in_class, naming: Option<DeclId> }
// `naming`: the class a class-scope candidate set was found in, for the
// access check of [expr.new]/19 and [expr.delete]/12 (WP-335).
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
plus canonical type. The visible set (`conversion_functions_all`, clang
`CollectVisibleConversions`; WP-779) walks the class, then each base in
declaration order, depth first: a base's conversion function is hidden
only when a class derived from it *on the same path* declares one to the
same canonical type, so two sibling bases' `operator int` are both
candidates (an ambiguous conversion); one reached through a virtual base
is hidden when it is hidden along any path, whatever the base order
(`struct D : B, C` and `struct D2 : C, B` with `B, C : virtual A` and only
`B` redeclaring: `B`'s wins in both); a function reached along several
paths is one candidate. `has_conversion_functions` (`conv/mod.rs`; the
array-new bound check and the static_cast "without a conversion operator"
wording) asks the same set, so an inherited or using-declared conversion
function counts (`tests/conformance/class.conv.fct/inherited-*.cpp`,
`expr.new/array-size-*.cpp`, `expr.static.cast/inherited-conversion-no-match.cpp`).
A conversion function a using-declaration names in
the class (`using B::operator T;`) is a candidate found as its shadow, so
its access is the using-declaration's ([namespace.udecl]/1, /19): a
private base's conversion made public that way is accessible (WP-726,
`tests/conformance/namespace.udecl/conversion-function-via-using*.cpp`). `explicit` ones apply to direct-init, explicit casts
and contextual bool. `contextually_convert_to_bool(e)` becomes
`resolve_conversion(bool, e, ToNonClass, ALLOW_EXPLICIT)` (as `bool t(e)`):
`if (w)` accepts `explicit operator bool`, `bool k = w;` is "no viable
conversion from 'W' to 'bool'" + "explicit conversion function is not a
candidate". Contextual implicit conversion ([conv]/6: `switch`, `delete`,
pre-C++14 array-new size) is `contextual_class_conversion(kind, e, loc)`
(`switch_conv.rs`, sema.md `switch`): the non-explicit conversion
functions whose result `kind` accepts must agree on one target, then
overload resolution picks one; several targets or an ambiguity is
clang's `err_*_multiple_conversions`/`ambiguous` + one note per function.
A conversion function asked to convert an object of class `S` to a class
target `T` where `S` is `T` or derived from `T` is a non-viable candidate
(`CandidateFailure::TrivialConversion`, clang `ovl_fail_trivial_conversion`;
[class.conv.fct]/1, [over.ics.user]/4), checked after the object argument,
the constraints and `explicit`, before the result: the copy or move
constructor initializes such an object (spec/sema.md §6, WP-609). Overload
resolution instantiates only the declaration of a conversion function
template specialization (deduction and substitution of its type; a
deduced return type is the one exception that needs the definition,
[dcl.spec.auto]/12), never its definition; the definition is instantiated
when the selected specialization is odr-used ([temp.inst]/11).
-Wclass-conversion ("will never be used") is reported only for a
conversion function declared outside any template instantiation (clang
suppresses it for instantiated members).

### 6.7 Literal operators and user-defined literals ([over.literal], [lex.ext]; `literal_op.rs`)
- **Declarations.** A literal-operator-id is `operator "" X` or
  `operator ""X`. Adjacent string literals concatenate; the result must
  be empty and must have no encoding prefix. The `Declarator` carries
  `lit_suffix`, and the function's name is
  `DeclName::literal_operator(X)`. `check_literal_operator_decl` mirrors
  clang's `CheckLiteralOperatorDeclaration`, and its first failure marks
  the decl invalid. In order:
  - it must not be a class member;
  - it must not have `extern "C"` linkage, which comes with a note at the
    linkage specification;
  - a template has no function parameters, and its head is one of:
    - `<char...>`;
    - one non-pack class-type NTTP;
    - GNU `<class C, C...>`, which also gives an ExtWarn;
  - a non-template has one parameter of type `unsigned long long`,
    `long double`, a character type or `const char *`, or it has two
    parameters, `(const C *, size_t)`; a mismatch gets clang's
    "did you mean" type;
  - no parameter has a default argument.

  A suffix that does not start with `_`, or that contains `__`, gets
  the `-Wuser-defined-literals` reserved warning.
- **Calls.** Unqualified ordinary lookup of `operator""X` picks the form
  in this order:
  - **integer:** `unsigned long long`, with the value too large to fit
    being an error; then raw `const char *`, with the spelling minus the
    suffix as a string literal; then `template<char...>`, with one
    `char` template argument per spelled character;
  - **floating:** the same, with `long double` re-classified as an
    `L`-suffixed literal;
  - **character:** the literal's own type;
  - **string:** in [lex.ext]/5 order:
    - a class-NTTP template called as `operator""X<str>()`;
    - the GNU pack, with the element type and then the code units;
    - `(str, len)`.

  A plain function call goes through `act_on_call`, so overload
  resolution and access checks apply. A template form calls through an
  `UnresolvedLookupExpr` with explicit template arguments.
- **AST deviation.** The result is a plain `CallExpr`. Clang's is a
  `UserDefinedLiteral`, a `CallExpr` subclass whose dump differs only in
  the node name. IR and mangling (`li <source-name>`) match clang
  (tests/codegen/user_defined_literals.cpp).

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
out-of-class members of class templates may not add defaults.
A friend function declaration with a default argument must be a
definition and the only declaration of the function in the translation
unit ([dcl.fct.default]/4; clang `ActOnFriendFunctionDecl`,
`MergeCXXFunctionDecl`; WP-769): a written friend declaration with one
that redeclares an earlier declaration is "friend declaration specifying
a default argument must be the only declaration" (note at the previous
declaration), one that declares the function first but is not a
definition is "friend declaration specifying a default argument must be
a definition" (`check_friend_default_args`, class/friend.rs; checked as
written, not at instantiation); a later redeclaration of a function
first declared by a friend declaration with a default argument is the
first error at the later declaration's name. **Calls**:
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
defaults. A static member function named by a class member access
(`o.f()`, `p->f()`, `this->f()`, also when found at instantiation through
a dependent base) is a named callee exactly as `f()`/`S::f()` is: its
omitted trailing parameters take their (lazily instantiated) defaults,
and the format-attribute and immediate-invocation checks apply
(`act_on_call`'s `named` callee; WP-555).

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

### 9.1 New diagnostic ids (`kiln_diag/src/ids/*.rs`, clang wording)
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

The name in "call to deleted function '{0}'" (and the other call
diagnostics) is the name as looked up, without qualifier or template
arguments (clang `ULE->getName()`): `show<int>(2)` and `ns::r(1)` print
'show' and 'r', not the selected specialization.

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
name; implicit special members at the class name. An implicitly
instantiated class template specialization is located at its pattern's
definition (the primary template's or the matched partial specialization's
name, clang `InstantiateClass`), not at the first declaration of the
template, so its implicit members' notes point there; an explicit
specialization or explicit instantiation keeps its own template-name
(WP-366).

**Parameter sugar** (WP-227): a bad-conversion note names the parameter
type as declared, sugar and top-level `const` kept (`no known conversion
from 'int' to 'WI' (aka 'W<int>')`, `'CI *const' (aka 'const int
*const')`), when it is the canonical target of the failed conversion
(`overload/diag.rs` `sugared_bad_param`); a target computed otherwise
(an object argument, a deduced type) is printed as converted.

### 9.3 Deviations from clang 19 (authoritative)
1. `-fshow-overloads=best` unsupported (always `all`).
2. No CUDA, `target` multiversioning, ObjC, or C
   `__attribute__((overloadable))`. (`enable_if` and `diagnose_if` are
   supported: §4.3, spec/sema.md §4.)
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
  likewise). For the single-candidate overload path the argument's ICS
  is computed as a candidate argument: a bad one (including a class
  parameter whose converting constructor is explicit or missing) gives
  "no matching function" with the `bad_conversion_detail` note, using
  the Bad ICS's to-type (WP-161). The error is located at the callee's
  begin, which for a qualified name is its nested-name-specifier
  (`set_overload_begin`, `set_template_id_begin`).
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
  `add_elision_candidates` (`class/ctor_select.rs`) adds the source's
  conversion functions *and conversion function templates*
  (`conversion_functions_all`; clang `AddTemplateConversionCandidate` with
  `AllowResultConversion` false): a template is deduced for `cv T` and its
  specialization kept only when it yields exactly `cv T` (not a reference);
  explicit ones only in a direct-initialization. Its object argument is an
  exact match, so it beats a converting constructor template that binds
  the source's base by derived-to-base conversion (libc++ tuple
  PR31384.pass.cpp; `over.match.best/conversion-template-elision-beats-converting-ctor.cpp`),
  and a deleted explicit template makes `T x(s)` ill-formed while `T x = s`
  is fine (`is_constructible` false, `is_convertible` true: libc++ tuple
  convert_move.pass.cpp `BonkersBananas`).
- **`IcsOptions::CANDIDATE_ARG`**: an argument of a candidate. In a
  reference binding, a deleted best [over.match.ref] conversion is no
  conversion (clang `FindConversionForRefInit`), so the candidate is not
  viable; a declaration's initialization still diagnoses the deleted
  function.
- **[over.match.list].** `init_list_ctor_init` (`init.rs`) runs phase 1
  as overload resolution over the initializer-list constructors and
  constructor templates (a template's dependent `initializer_list<U>` is
  recognized through `initializer_list_param` and `U` deduced from the
  list), with the whole list as the one argument; an empty list with a
  default constructor value-initializes. When nothing is viable,
  `construct_class` runs phase 2 over all constructors with the elements;
  a nested braced-init-list element is a placeholder argument
  (`act_on_braced_arg`) as in a call (WP-169). The /3.1.1 bit is
  `StandardConversion::std_init_list` (set on the `std::initializer_list`
  row of [over.ics.list], for a standard or user-defined sequence);
  `compare_ics` checks it after the /2 class and before /3.2, so
  `S({1, 2})` prefers `S(initializer_list<int>)` to
  `S(const int (&)[2])` (clang). The array-bound tie-breaker of /3.1.2
  (WP-179) is `StandardConversion::list_array` (`ListArray`: element
  type, number of elements initialized, unknown bound; clang's
  `InitializerListContainerType`), set on the array row of
  [over.ics.list] (an array of unknown bound counts one element per
  initializer) and kept through a reference binding. Right after /3.1.1,
  two sequences converting to arrays whose element types agree without
  cv-qualifiers compare by element count (fewer is better), then the one
  not converting to an array of unknown bound wins
  (`tests/conformance/over.ics.rank/list-array-bound.cpp`). Binding a
  reference to an array of unknown bound (`const int (&)[]`) to an
  array of known bound is not implemented yet ([dcl.init.ref]/4 with
  P0388's array conversions), so a call that selects such an overload
  still fails after overload resolution.
- **Casts.** `static_cast` / functional / C-style casts from a class to a
  non-class type run the direct-initialization's user-defined conversion
  (explicit conversion functions are candidates) with clang's
  `tryDiagnoseOverloadedCast` errors (`sema_ovl_*_conversion_in_cast`).
- **Accounting (§1.4).** `Sema::overload_stats()` returns the `ovl.*`
  counters (`OverloadStats`); `Sema::overload_memory()` the
  `sema.ovl.*` rows, and `Sema::memory()` (WP-663) sema's rows outside
  the AST, which `-fkiln-memory-report` prints (spec/driver.md) after the
  AST's and before the Oven's. The driver does not print `ovl.*`, so
  `cargo xtask bench` records the `overload` group's wall time and peak
  RSS but not those counters (an open item). `sema.ovl.conv_types` and
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
  `noexcept` ([except.spec]/9). An `operator new` function
  template ([temp.over]/1), in class scope or global (WP-730), is a candidate:
  with templates found, the functions and the templates' deduced
  specializations for `(size_t[, align_val_t], placement...)` go through
  overload resolution together ([expr.new]/20; `resolve_set`) and the
  chosen specialization is instantiated on use (WP-686, WP-712); with
  no viable candidate the notes are the overload set's, template
  candidates included. Without templates, `find_operator_new` matches
  each placement argument against its parameter (`placement_arg_match`):
  the same type is exact, and arithmetic conversions, `nullptr`,
  object pointer to `void *`, array decay and derived-to-base pointer
  conversion ([conv.ptr]/3, with no loss of cv) are viable but not exact.
  The most exact matches win (WP-801,
  `tests/conformance/expr.new/p19-placement-arg-derived-to-base-pointer-run.cpp`).
  Gap: this is not [over.ics.rank], so `B *` to `A *` does not beat
  `B *` to `void *`. A placement `operator delete` template matches
  when deduction against `void(void*, P...)` from the allocation
  function's parameters succeeds ([expr.new]/28, WP-713). The allocation function, the matching deallocation
  function of a new-expression and the deallocation function of a
  delete-expression are odr-used at the expression's location, so a member
  of a class template specialization is instantiated there ([temp.inst]/11;
  WP-686).
- **Gaps (owners).** Reversed and rewritten operator function template
  candidates are not formed (see §10.7). Access
  checks on class `operator new`/`delete`: sema.md §5.9 (WP-457). Destroying
  `operator delete` is selected per sema.md §5.9 (WP-336). `x.operator+=(2)`, `x.operator()(1)`,
  `x.operator[](0)` and `x.operator T()` (member access naming an
  operator- or conversion-function-id) resolve as the operator syntax
  does, in templates too (verified by WP-457 triage).
  No `note_equality_comparison_to_assign` ("use '=' to turn this equality
  comparison into an assignment") anywhere in kiln (sema follow-up).
  `aka` of a template specialization with an expression argument prints
  `'T<0 + 1>' (aka 'T<1>')` where clang prints no `aka` (types/print).
  `ovl.*` counters are not printed by the driver (§10.5).
- **Landed (WP-082).** The built-in `<=>` on scalars
  ([expr.spaceship]) yields the comparison category, or reports
  `sema_cmp_category_not_found` as clang does. The `adl` flag is set on
  a `CXXOperatorCallExpr` whose operator was found only by ADL (not yet on
  ordinary calls). A defaulted comparison is kept out of the candidate
  sets of its own subobject comparisons (`OverloadCandidateSet.exclude`).
  Those comparisons use the operator lookups captured at its
  `= default`. Selecting a deleted defaulted comparison reports
  `sema_ovl_deleted_comparison` (`object of type 'A' cannot be compared
  because its 'operator==' is implicitly deleted`) followed by the
  analyzer's notes (sema-class.md §4.12).

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
  does (`diag.rs::spec_desc`): the failing parameter is non-dependent in
  the template, or made non-dependent by the explicit template arguments
  (`deduce_spec` records the latter as `Candidate::explicit_nondep`, one
  bit per parameter below 64), so it was checked before the
  specialization was formed.
- Not yet:
  - `ics.rs` (conversion function templates);
  - `arrow.rs` (`operator->` templates);
  - a diagnostic for `&f` when no template matches the target type. That
    case still reports "not yet implemented", where clang prints
    `address of overloaded function` with deduction notes.
- Address of a function template ([temp.deduct.funcaddr]):
  - `resolve_single_template_id` resolves `&f<int>` when exactly one
    template accepts the explicit args. The parser calls it for a
    template-id not followed by `(`; template instantiation calls it again
    for an uncalled template-id whose arguments were dependent
    (`auto *p = &f<T>;`, [over.over]/3, [temp.arg.explicit]/4; WP-276).
    The same holds for a member template-id of a dependent class
    (`return D<I>::template f<F>;`, a `DependentScopeDeclRefExpr`): once
    substituted it is formed as the parser forms `C::f<A>` (every
    overload of the name, `widen_function_template_id`; the qualifier and
    begin, so `&C::template m<A>` is a pointer to member) and resolved
    unless it is a call's callee (`Substituter::as_callee`), where the call
    deduces the remaining arguments (WP-296). WP-297 needed the same for
    libc++ variant's relational operators (`relops_bool_conv.verify.cpp`:
    the uncalled `__make_dispatch` must resolve for the static_assert to
    fire; `over.over/qualified-template-id-in-template.cpp`).
  - When no non-template matches the target, `address_template_candidates`
    deduces P (the template's function type) from A (the target type)
    through `deduce_spec(.., target)`. It keeps the specializations that
    match exactly, then the one more specialized than all the others.
  - A specialization's dependent noexcept-specifier that deduction left
    uninstantiated (DR1330) is resolved before its type is compared.
  - The conversion sequence of an overload-set argument (`address_match`,
    used by `overload_set_ics`) selects in the same way, so `&f` for a
    function template converts to each candidate's function-pointer
    parameter through that parameter's own deduced specialization
    (WP-280, `over.over/template-address-overloaded-callee.cpp`). Clang 19
    rejects some of these calls where the targets differ only in
    noexcept (`o(&u)` with `o(void(*)(int) noexcept)` /
    `o(void(*)(int))`); clang 22 accepts them, and so does kiln
    (deliberately kept: the clang 19 behaviour is a bug fixed upstream).
  - Deduction for the address ignores a non-deduced exception
    specification (`TdfFlags::IGNORE_NONDEDUCED_NOEXCEPT` at the top
    level of `deduce_spec`'s `FuncAddr` step; clang's
    `isSameOrCompatibleFunctionType`), and
    `address_matches_ignoring_exc` accepts the specialization whose type
    equals the target but for the exception specification (WP-290,
    `over.over/template-address-noexcept-mismatch.cpp`). The conversion
    that follows checks it, as clang does:
    - initialization: `int (*p)(long) noexcept = &t;` with a potentially
      throwing `t<long>` is "cannot initialize a variable of type 'int
      (*)(long) noexcept' with an rvalue of type 'int (*)(long)':
      different exception specifications" (an lvalue of the function
      type without `&`; a written dependent specifier prints as
      instantiated text, `noexcept(sizeof(int) > 4)`);
    - simple assignment (`conv::implicit_convert`, `ConvCtx::Assignment`):
      the overload set is resolved silently and a selection that does
      not convert is `diagnose_overload_assign`'s "assigning to 'T' from
      incompatible type '<overloaded function type>'" at the operand,
      with every candidate noted (`overload_assign_notes`: a
      non-template's type mismatch note against a function target,
      otherwise "candidate function" / "candidate function template";
      a template-id notes only the templates);
    - call argument: `overload_set_ics` keeps the argument an overload
      set when the selection does not convert, and the candidate note is
      clang's "no overload of 't' matching 'T' for 1st argument"
      (`bad_conversion_detail` with the argument expression).
  - A function template-id resolved early to its one specialization
    (`resolve_single_template_id`, the parser's template-id not followed
    by `(`) is recorded in `ovl.template_id_refs` (the `DeclRefExpr` to
    its `UnresolvedLookupExpr`); `overload_set_of` sees through it, so
    `o(&t<int>)` and `p = t<long>` read as overload sets in those two
    diagnostics, as in clang.
  - [over.over]/1, /3 (WP-802): when that specialization does not
    convert in an initialization, direct-initialization, reference
    binding or `return` to a pointer or reference to function or a
    pointer to member function, the set is re-resolved against the
    target (`retarget_template_id`): every template of the set gets the
    explicit arguments and deduces the rest from the target, so
    `void (*p)(void*, int) = &wrap<1>` selects
    `template<int N, class T> void wrap(void*, T)` over the
    `template<int N> void wrap(void*)` the explicit arguments alone
    complete. Only when the target selects nothing is it
    `template_id_no_match`'s "address of overloaded function does not
    match required type".
    The specialization resolved early is then not used: when that
    resolution alone queued its definition, the definition is withdrawn
    from the queue ([temp.inst]/5), so an ill-formed body of the
    unselected `wrap<1>(void*)` is never instantiated (WP-802,
    `tests/conformance/over.over/template-id-retarget-unselected-not-instantiated-run.cpp`).

### 10.8 Landed (WP-106): the rest of the template candidates
- Conversion-function templates: `build_conversion_set` /
  `conversion_functions_all` (`ics.rs`) add `deduce_conversion`
  specializations ([temp.deduct.conv], `DeduceTarget`);
  `order_conversion_templates` applies /2.5 with
  `more_specialized_conversion`.
- For a reference target `U cv &` (`conversion_deduction_pair`), A is
  the referee with its cv when the template's result is a reference
  (/5: A may be more cv-qualified), else P and A both drop their cv
  (clang's workaround of a defect in /4). So `operator T const &()`
  deduces `T = U` for `U const &` and is an [over.match.ref] candidate
  binding directly to the returned lvalue, not one more way to a
  temporary that a deleted `operator T()` makes ambiguous (WP-704;
  test `tests/conformance/temp.deduct.conv/reference-target-keeps-cv.cpp`).
- `operator->` templates (`arrow.rs`): clang adds invalid `operator->`
  declarations as candidates, so `declares_arrow` decides between "no
  member named" and "no viable".
- Reversed and rewritten operator templates ([over.match.oper]/3.4):
  `add_rewritten_template`; ordering by `more_specialized_operator`
  ([temp.func.order]/3, object parameter and reversal) through
  `more_specialized_candidate`; P2468 `functions_correspond` maps
  specializations to their patterns. Operators no longer set
  `skipped_templates` for reversed or rewritten templates.
- `&f` against a target type with no matching template prints clang's
  `address of overloaded function` with `note_ovl_candidate_bad_deduction`
  notes (`address.rs`).
- [over.ics.list]: `drop_explicit_ctors`; `ctor_arg_info` gives braced
  arguments list `ArgInfo` (`T t({1})`); `init_list_ctor_init` is
  [over.match.list] phase 1.
- Gap: [temp.func.order]/6 (constraints of reversed candidates).

## 11. Agent parallelization (WP-068 split)
WP-068 has more than five deliverables, so it splits. 068a merges §10.1–10.3
types first (interface PR, docs/AGENTS.md); 068b starts from that.

| WP | owns (files) | deliverables | acceptance |
|---|---|---|---|
| **068a** core | `overload/{mod,ics,best,diag,adl,call}.rs`; edits to `conv/{scs,mod}.rs`, `expr.rs` (ULE/UME, `act_on_call`), `lookup.rs` (order, buffer), `init.rs`; non-operator ids of §9.1 | named and member calls with ADL; ICS (std/ref/user/ellipsis/list); §4; ctor/copy/conv/ref/list init (§2.6–2.7); default args (§7); §8 call/ctor/conversion shapes; §9.2; counters | conformance `over.match.{funcs,call,viable,best,ctor,copy,conv,ref,list}`, `over.ics.{user,ref,list,rank}`, `over.over`, `basic.lookup.argdep`, `dcl.fct.default`, `class.conv.fct`; ast goldens `ast/over.*`; `conv_ics.rs` cases for /3.2.3–3.2.6; overload bench recorded |
| **068b** operators | `overload/{oper,builtin,arrow,alloc}.rs`; `expr.rs` one dispatch call per binary/unary/subscript site; `decl.rs` `check_operator_decl`; operator ids | §2.4–2.5, §5, §6, surrogates, memoized built-in tables | `over.match.oper`, `over.built`, `over.call.object`, `over.oper`, `over.ref`, `class.compare` (rewrites), `expr.new` (lookup); 144/121-note relational ambiguity per target; `__float128` per target |

Shared `expr.rs` edits are disjoint (068b: one dispatch call per site), as
are the diag-id blocks. **Bench** `bench/overload/` (reference-workload-style): 200
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
