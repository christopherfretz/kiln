# Classes — `kiln_sema` (classes), `kiln_abi` (layout, vtables)

Status: **design** (WP-S3C). Implemented by WP-066a/b/c, WP-067a/b and
WP-078a/b (§9); consumed by WP-065 (parser), WP-076/077 (Oven) and WP-079/080
(codegen). Companions: `sema.md` (M2 sema), `sema-conv.md` (conversions,
`find_base`), `sema-overload.md` (WP-S3O: constructor and conversion-function
selection, implicit object parameter ranking), `sema-templates.md` (WP-S3T),
`ast.md`, `types.md` §11, `abi.md`, `oven.md`. Standard references are to the
C++26 draft; "clang" means clang 19 with `-std=c++26` on the container target
(aarch64 Linux, Itanium C++ ABI), the behavioural oracle.

## 1. Scope and principles

| | M3 (this spec) | M4 or later |
|---|---|---|
| definitions | all member kinds, injected-class-name, nested/local classes, static data members, `mutable`, `[[no_unique_address]]` (layout already M2) | `using` members beyond what WP-062 provides |
| access | [class.access] complete: specifiers, base access, friends, protected rule, nested classes, delayed checks | access in templates beyond WP-070's instantiation-context rule |
| bases | non-virtual (WP-066/067), virtual (WP-078), `final`, `override`, abstract classes, covariance | |
| special members | all six: implicit declaration/definition, triviality, deletedness, `constexpr`, `= default`/`= delete`, inheriting ctors (after WP-062); defaulted comparisons ([class.compare], WP-082, §4.12) | trivially relocatable (P2786) |
| construction | ctor-initializers, delegating ctors, NSDMI, temporaries, cleanups, lifetime extension incl. P2718 range-for, guaranteed elision, NRVO marking, aggregates with bases, paren-aggregate init | function-try-blocks and EH cleanups (exceptions milestone) |
| layout | full Itanium: bases, vptr, vbases, primary base, EBO, tail padding, bit-fields across bases, `#pragma pack`, `alignas`; vtable layout and VTT | MS ABI, `#pragma ms_struct`, `-fclang-abi-compat` |
| RTTI | `dynamic_cast`/`typeid` sema; final overriders for the Oven | |
| other | explicit object parameters (P0847): declarations, calls, `&C::f` (§2.5), lambdas (spec/sema-lambda.md §3.1, WP-356) | |

Principles:
1. **clang's model where the standard leaves a choice** (node kinds,
   implicit-member laziness, cast kinds, cleanups, NRVO marking), so `expect:
   ast` goldens compare against `-ast-dump=json` through the normalizer without
   special cases. Departures: "Deviations from clang 19".
2. **No per-class heap.** Members stay in the `DeclArena` chain; class facts
   are `RecordData.defn` bits; rare data (friends, overridden methods,
   ctor-initializers, key function, pack value, deleting-dtor `operator
   delete`) lives in shared arenas (`ListId`) or cold maps keyed by `DeclId`.
3. **O(1) lookups** in one class through the `(ctx, DeclName)` table
   (ctors/dtors/conversions by `DeclName` tags 2/3/4); only base traversal
   scales with the hierarchy (§3.2).
4. **Lazy**: implicit members declared on first lookup (§4.1), defined on
   odr-use or Oven demand; layouts, final overriders and vtables computed on
   demand and cached insert-only.
5. **Write-once**: a class is frozen at `}` except the §8.2 slots.
6. **One lowering**: implicit definitions get real bodies exactly as clang
   builds them, so the Oven (WP-076) and codegen (WP-079) lower the same AST.
   Subobject destruction ([class.dtor]/13) is derived from the class shape, not
   stored.
7. **Accounted**: new arenas/maps are `HeapSize` rows in `Ast::memory()`
   (`ctor_inits`, `friends`, `overrides`, `virtuals`, `vtables`,
   `lifetime_temps`); transient sema state reports its peak in
   `Sema::memory()`.

## 2. Class definitions

### 2.1 Definition lifecycle (parser → sema)
1. `act_on_tag` (M2) creates or redeclares the `CXXRecord`.
2. `act_on_start_record_definition(r)` (M2): sets `BEING_DEFINED`, pushes a
   `Record` scope, appends the injected-class-name (§2.2), and now also pushes
   a `ClassBuild` (§8.3) with the default access (`private` for `class`, else
   `public`). An implicit instantiation first takes the class-key of the
   definition it instantiates (`DeclArena::set_definition_tag`): a
   specialization named through `template<class T> class S;` and
   instantiated from `template<class T> struct S {...}` has public members
   and bases (libstdc++ 16 `<format>` `_Scanner`; WP-525,
   `class.access/class-key-of-template-definition.cpp`). An explicit or
   partial specialization takes the class-key of its OWN class-head, never
   the primary template's (the two may differ, [temp.expl.spec],
   [dcl.type.elab]/3): `template <class L> class T {...}; template <>
   struct T<double> : T<float> {};` inherits publicly and has public
   leading members, and `class` over a `struct` primary makes both private
   ([class.access.base]/2, [class.access]/3). `act_on_explicit_class_spec`
   and the partial-specialization redeclaration path re-key a
   specialization not yet defined at each declaring class-head
   (`take_spec_class_key`), so the defining head's key wins also over a
   reference made first (`T<int> *p;`) or a declaration with the other key
   (`template <> class T<int>; template <> struct T<int> {...};`); a
   union/class mismatch is never re-keyed. A specialization implicitly
   instantiated from a partial specialization takes that partial
   specialization's key as above (WP-611,
   `temp.expl.spec/class-key-of-specialization-{run,errors}.cpp`,
   `temp.spec.partial.general/class-key-of-partial-specialization-run.cpp`).
   2a. `act_on_class_final(r, loc)` (WP-065) when the class-head has a
   class-virt-specifier `final` ([class.pre]/1): sets
   `DefinitionBits::FINAL` on the `ClassBuild`, merged into the record's
   definition data at `}`.
3. `act_on_base_specifiers(r, &[BaseSpecInfo])` (new, §3.1) — once, before any
   member, even when the clause is absent (empty slice).
4. Member declarations in order (§2.3): `act_on_access_spec`,
   `act_on_declarator` with `DeclaratorContext::Member`, `act_on_friend`,
   `act_on_defaulted`/`act_on_deleted`.
5. `act_on_finish_record_definition(r, then_semi)` at `}`: the class is
   complete ([class.mem]/8). Sema computes the definition bits (§2.9), the
   abstract/override/hiding checks (§3.4), the key function, builds the friend
   list, declares the eagerly-declared implicit members (§4.1), resolves
   explicitly-defaulted-on-first-declaration members' triviality and
   deletedness, and clears `BEING_DEFINED`. No layout.
6. If `r` is not nested in another class being defined, the parser replays the
   complete-class contexts (§2.7) and then calls
   `act_on_finish_class_contexts(r)` (new), which resolves deferred exception
   specifications of explicitly defaulted members and runs the checks clang
   runs in `ActOnFinishCXXNonNestedClass`.

### 2.2 Injected-class-name ([class.pre]/2)
A `CXXRecord` decl with flags `IMPLICIT | INJECTED`, the class as its context,
first in the member list (dump: `CXXRecordDecl implicit referenced struct S`,
`implicit` mode only). It is a type name in lookup; `S::S` in a qualified name
that would name a type names the constructor instead ([class.qual]/2); a
mem-initializer-id naming it denotes a delegating constructor. Access is
public.

### 2.3 Member declarations ([class.mem])
| member | decl kind | notes |
|---|---|---|
| non-static data member | `Field` | access from `ClassBuild`; `mutable` → `FieldData::MUTABLE`, `HAS_MUTABLE_FIELDS`; `[[no_unique_address]]` (M2); NSDMI style bits set now, `init` written at replay (§2.7) |
| static data member | `Var`, `VarStorage::Static`, ctx = class | in-class: a declaration unless `inline` or `constexpr` (implicitly inline, [dcl.constexpr]/1); a `static const` integral/enum member with an initializer is usable in constant expressions ([class.static.data]/4: the initializer must be a constant expression of any value of the member's type, `unsigned __int128` above `INT128_MAX` included, WP-275, else `in-class initializer for static data member is not a constant expression`); `int S::s = 1;` is a redeclaration (lexical ctx = namespace, `HAS_LEXICAL_CTX`) |
| member function | `CXXMethod` | in-class definition implicitly inline; `static` → `FnStorage::Static`; cv/ref-qualifiers into `FuncProto.this_quals`/`ref_qual` (types.md §5.4); `virtual`, `= 0`, `override`, `final` (§3.4) |
| constructor / destructor / conversion | `CXXConstructor` / `CXXDestructor` / `CXXConversion` | `DeclName` tag 2/3/4 keyed by the canonical class `TypeId` / target `QualType` (ast.md §3) |
| nested class / enum / typedef / alias | M2 kinds, ctx = class | access applies to the name; nested class is a member ([class.access.nest]) |
| friend | `Friend` (payload: friend decl or type) | §2.4; a friend-type-specifier-list (`friend A, Ts...;`, [class.friend]/3, P2893) declares one `Friend` per specifier through `act_on_friend_type`, a pack expansion one of type `PackExpansion(Ts)` expanded per element at instantiation (spec/sema-templates.md §7.1); a friend function first declared in a class is a member of the innermost enclosing namespace, hidden from ordinary lookup ([namespace.memdef]/3, [class.friend]/7): it is entered in that namespace's lookup table with `LookupFlags::ADL_ONLY` (sema-overload.md §2.2, decision 7), which ordinary lookup skips and ADL (WP-068a) finds; no side table. A later namespace-scope redeclaration is entered without the flag and is visible to ordinary lookup. A friend's redeclaration lookup in that namespace sees hidden friends but not the members of its inline namespaces (clang's `FilterLookupForScope` without `AllowInlineNamespace`), so a hidden friend `iter_move` does not clash with an inline-namespace CPO object of that name (WP-169) |
| access-specifier | `AccessSpec` | changes `ClassBuild.access`; kept for the dump |
| `using` member | WP-062 (`Using`, `UsingShadow`) | shadow access = the using-declaration's; inheriting constructors §4.10 |
| member template | WP-070 | |

Per-declaration checks: redeclaration of a member (`class
member cannot be redeclared`, `constructor cannot be redeclared`, `duplicate
member 'a'`), except that non-static data members named `_` are
name-independent ([basic.scope.scope]/5, P2169, WP-356; spec/sema.md
§5.24): several may be declared, a use through member access that finds
more than one is `sema_placeholder_ambiguous`; `mutable` with `const` or a reference; `constexpr` non-static
data member; incomplete or abstract field type (`field has incomplete type
'In'` + note `definition of 'In' is not complete until the closing '}'`);
a union member of reference type (`union member 'r' has reference type 'int
&'`, at the name, also substituted; [class.union.general]/2, WP-193; the
field's type becomes the error type);
static data member in a local class or in an anonymous/unnamed class;
`virtual`/`explicit`/pure on the wrong kind of member.

Friend functions of a local class ([class.friend]/11, WP-280, WP-290;
`local_friend.rs`): an unqualified friend function must redeclare a function
declared in the innermost enclosing non-class scope (the block holding the
class, its `Scope.locals`, matched by `pick_prev_function`); that declaration
is its previous one, so the merge checks apply (`exception specification in
declaration does not match previous declaration`, return types; not in a
templated class, whose friends wait for instantiation). Otherwise `no
matching function found in local scope`, the friend is `INVALID` and
redeclares nothing (`check_local_friend`; at the definition, also in a
function template). Clang's notes (`DiagnoseInvalidRedeclaration` with
`IsLocalFriend`): the enclosing blocks are searched outward to the
function's outermost one, stopping at the first declaring the name; each
function found there whose parameters are similar (same count, each the
same ignoring top-level cv, or after stripping pointers and references, or
classes/enumerations of the same name) gets `local declaration nearly
matches` at its name, or `type of Nth parameter of local declaration does
not match definition ('A' vs 'B')` at the first differing parameter's first
type-specifier (block-scope functions record it in `type_spec_locs`). When
no block declares the name, clang's typo correction runs over every
namespace-scope function (not templates, hidden friends or block-scope
declarations) that has no definition and similar parameters, whose name is
within `(len+2)/3` byte edits (a name under 3 bytes only exactly), with
`len/ed >= 3`: its distance is `(100*ed + 110*q + 50)/100` for `q` qualifier
components, and must also keep `len/distance >= 3` unless the name is the
friend's. The qualifier, from the enclosing namespace: `::` for the global
namespace (one component); `::A::B::` for an enclosing namespace, or when the
first relative component names an enclosing namespace; else the path from
the common namespace (`N::M::`); inline and unnamed namespaces are not
spelled. The least distance wins; a tie between names or namespaces suggests
nothing, and so does a winner the friend would not redeclare (another
parameter list or return type, or a non-throwing against a potentially
throwing exception specification): `no matching function 'f' found in local
scope; did you mean 'N::f'?` + `'N::f' declared here`. A using-declarator's
target is found as a namespace function. Operator friends (WP-329) match
block-scope operator declarations, which are recorded in `Scope.local_ops`
(`push_local_operator`; operator names are not in `Scope.locals`), get the
same near-match notes, and get no typo correction (the plain error when no
block declares the operator). Tests: `class.friend/local-class-friend-no-prior.cpp`,
`local-friend-near-match.cpp`, `local-friend-typo-correction.cpp`,
`local-friend-typo-correction-qualifiers.cpp`,
`local-friend-block-scope-operator.cpp`, `local-friend-access-run.cpp`.

Before the block is searched, the friend's redeclaration lookup (clang
`LookupLocalFriendName`, WP-329, `local_friend_member`) searches the
friend's own class and then each local class enclosing it, innermost
first ([class.friend]/11, [class.local]): each class's own members only
(`lookup_in_dn`; no base classes, nothing declared after the friend). The
first class declaring the name decides. A corresponding member function
(`pick_prev_function`) is befriended: the friend is declared as if
qualified by that class. A data member or other non-function gives
`redefinition of 'x' as different kind of symbol` at the friend's name +
`previous definition is here`. Any other overload set gives `no matching
function found in local scope` with no notes. Either error makes the
friend declare nothing. A class template's instantiation skips such a
friend in its member list, as it skips hidden friend definitions
(`class_body_with`). Test: `class.local/local-friend-enclosing-class-member.cpp`.

A friend function defined in a local class ([class.friend]/6, WP-329) gives
`friend function cannot be defined in a local class` at its name, after
the declaration is processed and only if the declaration is valid: not
after `no matching function found in local scope` or a mismatched
exception specification. The parser marks a friend definition
(`act_function_def_next` around `act_on_member`). In a function template
the error comes once, at the definition. Test:
`class.friend/local-friend-definition.cpp`.

**Invalid classes** (WP-406; clang `ActOnFields`, `setInvalidDecl` on the
record). A class being defined becomes invalid (`DeclFlags::INVALID` at its
`}`) when a field is invalid (an unknown or incomplete type, a negative
array bound or bit-field width, a `mutable` reference or const member), a
base is invalid, a member is a duplicate, or a field's type is (an array of)
an invalid class. An invalid anonymous struct or union declares nothing and
leaves its enclosing class valid. Uses of the class before its `}` that need
it complete stay quiet. An invalid class still gets its implicit special
members, and its layout is size and alignment 1 (abi.md §1, "Total"):
`sizeof`, `alignof` and `static_assert`s on it see 1 in sema and the Oven
alike. A
constant evaluation that would construct one fails without a note. Test:
`class.mem/invalid-member-class-size.cpp`.

Out-of-line member definitions (`void S::f() {}`): `act_on_declarator` with a
qualified declarator-id looks up the name in `S` only and picks the declaration
with the same parameter-type-list, `this_quals` and `ref_qual` (template heads:
WP-070). No match → `out-of-line definition of 'f' does not match any
declaration in 'S'`. Semantic ctx = class, lexical ctx = the enclosing
namespace; the body's scope chain is class scope (and its bases) then the
namespace of the definition.

### 2.4 Access control ([class.access])
**Data.** `DeclNode.misc.access` on every member; `BaseSpec.access` (`None` =
`private` for `class`, else `public`); cold map `friends: record → ListId`
(befriended functions' first decls and classes), built at `}`, used only by
access checks — friend name lookup goes through the namespace table (§2.3).
In a class nested in a class template (or a partial specialization),
`friend class X;`, `friend X;` and `friend class X<P...>;` naming the
enclosing template's injected-class-name or its own parameters befriend the
current instantiation ([temp.local]/1, [temp.dep.type]/1): the pattern is
in the list (so the pattern's members check against it), and each
instantiation befriends its own specialization only, whose members,
out-of-line member templates instantiated later included, get access
(libc++ `binomial_distribution::param_type`; tests/conformance/class.friend/
nested-befriends-enclosing-template*.cpp). An elaborated-type-specifier
`class X` naming the injected-class-name is the injected type `X<P...>`
anywhere inside the template, not only in a friend declaration.

A class first declared inside a template by an elaborated-type-specifier
that is a plain reference (a parameter or template-argument type `struct P*`,
`_If<C, struct __private_nat, It>`) or by `friend struct F;` is a member of
the nearest enclosing namespace ([basic.scope.pdecl]/7, [class.friend]/11),
so its type is not dependent ([temp.dep.type]/9): it is built with
`dependent_depth` cleared (`decl.rs` `namespace_tag_record`), overload
resolution and later definitions see one ordinary class, and an atomic
constraint naming it is a constant after substitution ([temp.constr.atomic]/3;
libc++ `__bit_iterator::operator=`, probed by `std::formattable<vector<bool>>`;
tests/conformance/temp.constr.atomic/elaborated-tag-in-template.cpp, WP-298).
A local class (an enclosing block) keeps the template's dependence.

**Rule** (`class/access.rs`, clang `SemaAccess`). Member `m` found in naming
class `N` ([class.access.base]/5) is accessible at context `R` iff: (a) `m` as
a member of `N` is public; (b) it is private and `R` is a member or friend of
`N`; (c) it is protected and `R` is a member or friend of `N`, or of a class
`P` derived from `N` in which `m` as a member of `P` is accessible, subject to
[class.protected]; or (d) a base `B` of `N` is accessible at `R` and `m` is
accessible at `R` named in `B`. "As a member of `N`" follows each base path
with [class.access.base]/1 (private members of a base are inaccessible; a
protected base demotes public to protected; a private base demotes to private);
the most permissive path wins. Members of `X` include nested and local classes
of `X` and their members ([class.local]/2). A friend function defined in a class `C` is in `C`'s scope ([class.friend]/7): its effective context continues from `C` outward, so it shares the access of `C` and of every enclosing class, including friendship granted to them; this holds for each specialization of a friend function template (the template's in-class declaration, not the namespace-scope specialization, names the lexical class). A friend defined outside the class gets none of this (WP-278; libc++ `join_view::__sentinel`'s `operator==` calling `__iterator::__get_outer()`; `tests/conformance/class.friend/friend-template-in-nested-class-access.cpp`). [class.protected]: for a non-static
protected member named through an object expression or `&P::m`, when only (c)
via `P` grants access the object's class must be `P` or derived from `P` (`'p'
is a protected member of 'S'` + note `can only access this member on an object
of type 'D'`; for a constructor or destructor, or a constructor template,
the note is `protected constructor can only be used to construct a base class
subobject` / `... destructor ... destroy ...`, clang `TryDiagnoseProtectedAccess`;
`class.protected/ctor-restricted-note.cpp`, WP-680). Private: `'x' is a private member of 'S'` + `declared private
here` / `implicitly declared private here`, located at the member's first
declaration (a member alias template at its `using` keyword, as clang
locates a `TypeAliasTemplateDecl`; WP-238).

**Checked**: every name found by class member or qualified lookup into a class
(for functions, the one selected by overload resolution), type names included
([class.access]/4, WP-174): a type or template name the parser commits to
(`A::Priv`, an unqualified class-scope name, `typename A::Priv`), each
nested-name-specifier component (`A::Priv::X`, and a template-id one,
`A::PT<int>::X`, before its `::`; WP-238), an elaborated reference
(`struct A::Priv *`), a base-specifier (checked at once in the deriving class's
context without its bases, as clang 19 does) and `T::type` after substitution
(a SFINAE failure in a SFINAE context; reported at the name as written,
spec/sema-templates.md §5.8, or at an alias template's `using` with clang's
access context, §4.10), a qualified member template name
(`A::PT<int>`), a mem-initializer-id, and each member a member
using-declarator names ([namespace.udecl]/19; named through an object of the
current class, as clang's `BuildUsingDeclaration` does, so `using B::m;` of a
protected member of a base is accessible). A using-declarator with a dependent
qualifier (`using T::f;`, `using typename T::type;`) is checked when the class
is instantiated, at the declarator's name (`member_using`,
`member_using_typename`; WP-194); inheriting constructors are not checked. The parser's classification
(`classify_name`) is side-effect free; `Sema::check_type_name_access` redoes
the lookup for a class member to learn the naming class and using-shadow. An
injected-class-name is a public member of its own class (`struct G : F2 {
A::B b; }` with `F2 : private A`: `'A' is a private member of 'A'` + `constrained
by private inheritance here` + `member is declared here`); selected
constructors, destructors and conversion functions; subobject members used by
implicit definitions (failure deletes the member, §4.2); derived-to-base
conversions (§3.3); `operator delete` of virtual dtors. Overriding ignores
access.

**Delayed checks.** Checks raised while parsing the decl-specifiers and first
declarator of a namespace- or block-scope simple-declaration (with the
template head before it, `template<A::I> struct A::Q`) queue in
`Sema.delayed_access` and resolve when the declaration exists, in the
[class.access]/7 context (`access_context_of`): for `S::B S::f(S::B)` the whole
declaration including the return type is checked as a member of `S`; a
function or class is its own context (so a befriended `A::I g(A::I)` or
`template<A::I> struct R` is accepted); a friend declaration is checked in the
befriending class. Only checks made in the context the delay point opened in
are queued: a class or function body (or lambda) inside the declaration checks
at once (clang's `pushUndelayed`). Member declarations are not delayed
(clang 19 rejects `friend A::I g(A::I);` in a class `A` does not befriend).
The delay point of an explicit instantiation's or explicit specialization's
declaration drops its checks instead ([temp.spec.general]/6,
`exempt_next_delayed_access`); its function body, initializer, base-clause and
member-specification are checked as usual, and so is any SFINAE substitution
it triggers (deduction of the specialization, default template arguments, the
substituted signature): checks made while the instantiation stack is in a
SFINAE context are never delayed, so they run in the template's context and
an inaccessible name is a substitution failure (`candidate template ignored:
substitution failure [with T = C, U = typename C::P]: 'P' is a private member
of 'C'`), for an explicit instantiation and for a call in any declaration's
declarator alike (clang 19, `-fclang-compat<22`:
tests/conformance/temp.explicit/access-in-substitution-clang19.cpp). From
clang 21 on (`-fclang-compat>=22`) the deduction of an explicit
specialization's or instantiation's candidates (`match_function_spec_in`)
runs with access checks suppressed (`inst.suppress_access`, clang
`SuppressAccessChecks`): an access error in its SFINAE context is no
substitution failure, only the note's reason when the candidate fails for
another reason (`int N = T::obj` with a non-constant `obj`); constraint
satisfaction still checks access (`satisfy` clears the flag). Deviation: a
later call reuses the specialization so formed, where clang 22 reports the
access error at the call; clang 22 also fails the instantiation after an
earlier failed call deduced the same arguments (order-dependent, not copied)
(tests/conformance/temp.explicit/access-in-substitution.cpp).
A default template argument that was formed but diagnosed is shown in the
note's argument list, as clang keeps it (`deduce_call.rs`). The template arguments of a
class-head `X<args>` that declares or defines a specialization (partial or
explicit) are dropped too ([temp.spec.partial.general]/10); a reference's join
the enclosing delay point (`end_delayed_access_to_enclosing`). In
template patterns: at instantiation (WP-070, §4.8 of sema-templates.md); in
SFINAE contexts: substitution failure (S3T). No result cache in M3 (Q6).

### 2.5 `this` and the implicit object parameter
- `CXXThisExpr`: prvalue `cv C*` (`cv` = `this_quals`; `C*` in an NSDMI).
  `FnState.this_ty`, set by member functions and NSDMI replay; lambdas capture
  it (WP-075). Elsewhere: `invalid use of 'this' outside of a non-static member
  function`.
- **Implicit member access** ([expr.prim.id.general]/3): a name of a non-static
  member where `this` is usable becomes `MemberExpr(implicit CXXThisExpr,
  ->m)`, `this` cast `UncheckedDerivedToBase` with the path when `m` is in a
  base (clang). Unevaluated operands need no object (`sizeof(S::x)`). Errors:
  `invalid use of non-static data member 'x'`, `invalid use of member 'x' in
  static member function`, `call to non-static member function without an
  object argument`. They are reported at the end of the full-expression
  (`flush_member_refs`); a constant expression (a non-type template
  argument) whose flush reported one is erroneous, with no "not a
  constant expression" after it (clang's `ExprError`). The operand of
  `decltype` is a full-expression ([intro.execution]/5): `decltype(C::f)`
  of a non-static member function is the "without an object" error there,
  so in a SFINAE context it is a substitution failure (libc++'s
  `tuple_size<const T>`; tests
  `temp.deduct.general/decltype-nonstatic-member-function.cpp`,
  `dcl.type.decltype/nonstatic-member-function-error.cpp`,
  `temp.arg.nontype/nonstatic-member-without-object.cpp`).
- **Through a dependent base** ([temp.dep.type]): in a member of a class
  template with a dependent base, a qualified name `B::m` that names a
  non-static member of a class that is not (yet) a base is kept as a
  dependent implicit member access `(*this).B::m`, resolved when the
  class is instantiated (`dependent_implicit_member`). If `B` is then
  not a base, the error is clang's "'B::m' is not a member of class
  'D<T>'". `&B::m` is a pointer to member, never an implicit access
  (the `&` operand undoes it).
- In a `const` member function, members through `this` are const unless
  `mutable` ([expr.ref]/6): `cannot assign to non-static data member within
  const member function 'f'` + `member function 'K::f' is declared const here`.
- `implicit_object_type(method)` gives `cv C&` / `cv C&&`
  ([over.match.funcs.general]/4) for S3O's ranking.
- A destructor is invokable on a `const`, `volatile` or `const volatile`
  object ([class.dtor]/5): binding the object argument of an explicit
  destructor call (`p->~T()` with `p` a `const T*`, libc++
  `std::destroy_at` on a const element) adds the object's cv to the
  destructor's, as clang's `TryObjectArgumentInitialization` does
  (`bind_object_argument`, WP-295;
  `tests/conformance/class.dtor/cv-object-explicit-call.cpp`).
- Explicit object parameters (P0847, WP-149): `ParamInfo::explicit_object`
  carries the `this` location; `act_on_function` sets FnSpec
  `explicit_object` (also on instantiation, via `ParamData::EXPLICIT_OBJECT`)
  after checking [dcl.fct]/6: only the first parameter
  (`sema_explicit_object_param_not_first`), not in a static, virtual or
  non-member function (`..._nonmember`), no cv- or ref-qualifier
  (`..._qualifier`, reported at the `)`, where clang points at the
  qualifier), no default argument, not a pack. `this` in the body is
  `sema_explicit_object_this`. `x.f(args)` / `p->f(args)` is a `CallExpr`
  of the decayed `DeclRefExpr CXXMethod` with the object as argument 0
  (converted like any argument; templates deduce `Self` from it);
  `&C::f` is an ordinary pointer to function ([expr.unary.op]/3.1), and
  `is_implicit_object_member` excludes such members from pointer-to-member
  matching ([over.over]). Codegen calls them directly
  (`is_explicit_object_method`). Lambdas with an explicit object
  parameter (the closure type as `Self`) follow spec/sema-lambda.md §3.1
  (WP-356): `param_nonmember` and `param_not_first` select "lambda".

### 2.6 Pointers to members
`&C::m` forms `MemberPointer` (types.md §5.2) for non-static members (`&(C::m)`
and unqualified `&m` are not member pointers, [expr.unary.op]/4); `.*`/`->*`
per [expr.mptr.oper]; conversions `DerivedToBaseMemberPointer` /
`BaseToDerivedMemberPointer` (sema-conv rank rules apply; virtual-base path is
ill-formed). The constant form is ast.md §9's `MemberPtr` scalar.

**Landed (WP-083): member-pointer declarators.** The parser emits
`Chunk::MemberPointer { qual: Option<Qualifier>, quals, loc }` (parse.md
§4) for `T C::* cv` in named, abstract and new-declarators; Sema
(`type_for_declarator_at`) takes the class from the nested-name-specifier's
last component (a `Nns::Type` that is a record or dependent, or a dependent
`Nns::Identifier`, which becomes a `DependentName`, so `T::*` stays
dependent) and checks, in clang's order and at the nested-name-specifier's
start: not a class (`'p' does not point into a class`; a typedef to a
non-class prefix is `'I' (aka 'int') is not a class, namespace, or
enumeration`), a reference pointee (`'p' declared as a member pointer to a
reference of type 'int &'`), and `void` (`'p' declared as a member pointer
to void`); abstract declarators name `'type name'`. A `qual: None` chunk
(the prefix error was already reported) makes the type the error type.
Function member pointers keep their cv/ref-qualifiers.
`sema_init_conversion_failed` gained a fifth argument, clang's
`HandleFunctionTypeMismatch` suffix (`fn_mismatch_suffix`: `different
classes ('D' vs 'C')`, number of parameters, Nth parameter, return type,
qualifiers, exception specification) for member-pointer and
pointer/reference-to-function pairs. The Oven's type table gives
`MemberPointer` the Itanium size and alignment (8 / 16, pointer-aligned)
so `sizeof`/`alignof`/layout agree with kiln_abi; the value itself stays
opaque to the evaluator (constant evaluation of member pointers is WP-076).
`[dcl.mptr]` tests: `tests/conformance/dcl.mptr/`.

**Landed (WP-085): member-pointer follow-ups.** `==`/`!=` on member
pointers (sema.md §5, "Member-pointer equality"); tentative parsing
accepts template-id components in a member-pointer
nested-name-specifier (`X<T>::*`, `X<T>::Y::*`, `::X<int>::*`,
`T::template W<int>::*`; parse.md §2.1); a nested-name-specifier
component that names something other than a class, namespace or
enumeration is `'x' is not a class, namespace, or enumeration` with a
`'x' declared here` note when ordinary lookup finds exactly one
declaration that is not a function template (clang's second lookup in
`BuildCXXNestedNameSpecifier`); and `out-of-line definition of 'f' does
not match any declaration in {1}` takes clang's scope text (`'S'` for a
class, `namespace 'N::M'`, `the global namespace`) and is now also
emitted for a qualified namespace-scope function that names no previous
declaration ([dcl.meaning.general]/2). A class context is printed as
its type, a current instantiation unqualified at compat 19 (`'A<T>'`)
and fully qualified at 22 (`'N::A<T>'`, `class_context_str`); at compat
22 the error for a class context carries "{0} defined here" at the
class (`with_class_defined_note`, WP-394), also for an out-of-line
constructor (WP-409). A member function definition that
differs from the declared member only in its return type is "return type
of out-of-line definition of 'C::g' differs from that in the
declaration" with "previous declaration is here" (WP-409). Tests:
`expr.eq/`, `dcl.mptr/template-id-qualifier.cpp`,
`basic.lookup.qual/not-a-scope.cpp`,
`dcl.meaning/qualified-no-match{,-clang22}.cpp`,
`dcl.meaning/qualified-no-match-class-note{,-clang19}.cpp`,
`dcl.meaning/out-of-line-return-type-mismatch.cpp`.

### 2.7 Complete-class contexts ([class.mem]/7)
The parser (WP-065) saves the tokens of member function bodies (with
ctor-initializers), default arguments, *noexcept-specifier*s, NSDMIs and
contract assertions. Until replay: parameters carry `ParamData` flag
`DEFAULT_ARG_PENDING` (and sit in `Sema.unparsed_default_args` with the
argument's location: a call that uses one gives `use of default argument
to function 'f' that is declared later in class 'S'` + `default argument
declared here`, clang `CheckCXXDefaultArgExpr`; WP-269), methods and
unqualified friends have `ExcSpec::Deferred(self)` and sit in
`Sema.unparsed_exc` (sema.md §4.7), fields have NSDMI style bits but `init == None`. After the
outermost class's `}` the parser replays in clang's order
(`ParseLexedMethodDeclarations`, `ParseLexedMemberInitializers`,
`ParseLexedMethodDefs`), each phase over the class and its nested classes in
declaration order:
1. default arguments → `act_on_param_default_arg(p, e)` (write-once; a
   braced one → `act_on_param_default_list(p, list)`, which
   list-initializes the parameter's type),
   noexcept-specifiers → `act_on_exception_spec(f, spec)`;
2. NSDMIs → `act_on_field_init(field, init)` (`this` usable; own
   full-expression, §5.3);
3. bodies → `act_on_start_function_def`, `act_on_mem_initializers` (§4.9),
   body, `act_on_finish_function_def`.

Delayed entry points (WP-065), which put Sema back into the scopes the
replayed tokens were written in:
- `act_on_start_delayed_class(r)` / `act_on_finish_delayed_class(r)`
  wrap each class in each phase that has items of that phase, nested
  classes inside their parent's (clang's `ReenterClassScopeRAII`). The
  stub pushes a `Record` scope for `r` and makes it the context; finish
  pops it and restores the lexical context.
- `act_on_start_delayed_method(f)` / `act_on_finish_delayed_method(f)`
  wrap phase 1 of one function (clang's
  `ActOnReenterCXXMethodParameter` and `ActOnFinishDelayedCXXMethodDeclaration`):
  a `Prototype` scope with all of `f`'s parameters visible. (clang makes
  a parameter visible only after its own default argument; the stub does
  not.)
- `act_on_start_field_init(field)` precedes each NSDMI (clang's
  `ActOnStartCXXInClassMemberInitializer`); `act_on_field_init` follows
  it only when the initializer parsed. `= {…}` is copy-list-initialization
  (an explicit constructor is `chosen constructor is explicit in
  copy-initialization`), `{…}` direct-list (`FieldData::init_style`, also
  copied to an instantiated field). An initializer that fails marks the
  field `INVALID`: constructors then skip it, and an aggregate
  initialization or `field_filler` using it recovers silently (clang's
  failed `BuildCXXDefaultInitExpr`).

Per outermost class the parser calls, after
`act_on_finish_record_definition(r, _)`: for each phase in order,
`act_on_start_delayed_class(r)`, the phase's items in declaration order
(a nested class's items wrapped in its own
`act_on_start/finish_delayed_class`), `act_on_finish_delayed_class(r)`;
then `act_on_finish_class_contexts(r)`. The pinned sequences are the
trace tests in `kiln_parse/src/class.rs` (parse.md §10).

Needing an unparsed NSDMI earlier (a nested class's implicit default ctor used
in the enclosing class) is an error: `default member initializer for 'x'
needed within definition of enclosing class 'S' outside of member functions`
(`sema_default_member_init_not_yet_parsed`, clang `BuildCXXDefaultInitExpr`)
at the use, `S` the outermost lexically enclosing class, with the note
`default member initializer declared here` at the field's name. The field
becomes invalid (outside SFINAE), so later uses of it are quiet: an aggregate
initialization recovers and an implicit constructor's definition skips it
(`define.rs` `collect_field_init`). An implicit default constructor's
exception specification ([except.spec]/8), resolved before the constructor is
defined (`mark_function_referenced`, also for `noexcept(X())` and the
`__is_nothrow_*` traits), meets the unparsed initializers first
(`excspec.rs` `exc_spec_needs_unparsed_inits`, clang's
`SpecialMemberExceptionSpecInfo`): each such field of the class is diagnosed
at the constructor's location, the diagnostic not deduplicated (one per
field), under the context note `in evaluation of exception
specification for 'W::X::X' needed here` at the use (`SynthKind::
ExceptionSpec`, note `note_exception_spec_evaluation_here`). Every implicit
exception specification evaluated pushes one such context (`resolve_exc`,
clang `EvaluateImplicitExceptionSpec`), a subobject's at the member or
base-specifier that calls it (clang `getSubobjectLoc`; clang points at the
base's type, kiln at the base-specifier, a follow-up), so a nested one
prints one note per level, innermost first (WP-238); the fields are
then invalid, so the definition, its constant evaluation and the enclosing
class's implicit default constructor raise nothing more. These fields are
reported before the bases are visited (clang visits the bases first).

### 2.8 Unions and variant members
[class.union]: at most one variant member has an NSDMI or appears in a
ctor-initializer (`initializing multiple members of union` + note `previous
initialization is here`); union special members are deleted when a variant
member's corresponding member is non-trivial ([class.union.general]/3, §4.2);
no bases, not a base, no virtual functions, no reference members. Anonymous
unions are M2 (`IndirectField`).

### 2.9 Definition data ([class.prop], [dcl.init.aggr])
Computed at `}` in one pass over bases and members (O(members + bases)).
Existing `DefinitionBits` 0–23 keep their meaning; WP-066a adds bits 24–49
(clang `DefinitionData` names in parentheses):

| bits | meaning |
|---|---|
| `DYNAMIC` | not stored: accessor `POLYMORPHIC` or `HAS_VIRTUAL_BASES` (`isDynamicClass`) |
| `USER_DECLARED_{COPY_CTOR,MOVE_CTOR,COPY_ASSIGN,MOVE_ASSIGN,DTOR}` | (`UserDeclaredSpecialMembers`) |
| `USER_PROVIDED_DEFAULT_CTOR` | (`user_provided`) |
| `HAS_IN_CLASS_INIT` | some field has an NSDMI |
| `HAS_INHERITED_CTOR`, `HAS_INHERITED_ASSIGN` | (WP-062 `using` of ctor / `operator=`) |
| `IMPLICIT_COPY_CTOR_CONST_PARAM`, `IMPLICIT_COPY_ASSIGN_CONST_PARAM` | (`implicit_has_const_param`, [class.copy.ctor]/7) |
| `DEFAULTED_DEFAULT_CTOR_CONSTEXPR` | (`defaulted_is_constexpr`) |
| `NEEDS_OVL_{COPY_CTOR,MOVE_CTOR,COPY_ASSIGN,MOVE_ASSIGN,DTOR}` | (`needs_overload_resolution`): a subobject's selection is not trivially known |
| `DEFAULTED_{DEFAULT_CTOR,COPY_CTOR,MOVE_CTOR,DTOR}_DELETED` | known-deleted without overload resolution (`DefaultedXIsDeleted`) |
| `HAS_UNINIT_REF_OR_CONST` | reference or const-without-default-init field without NSDMI |
| `HAS_PRIVATE_FIELDS`, `HAS_PROTECTED_FIELDS` | for aggregate/standard-layout |
| `POD` | [class.prop]/9 (dump `pod`; the Itanium POD-for-layout test stays in `kiln_abi`, abi.md §4) |
| `CAN_CONST_DEFAULT_INIT` | [dcl.init.general]/8 (`can_const_default_init`) |

`AGGREGATE` ([dcl.init.aggr]/1: no user-declared or inherited ctors, no
private/protected direct non-static data members, no virtual functions, no
virtual/private/protected bases), `POLYMORPHIC`, `ABSTRACT` (§3.4), `EMPTY`,
`STANDARD_LAYOUT` ([class.prop]/3), `TRIVIAL_*` (§4.3), `TRIVIALLY_COPYABLE`
([class.prop]/1, using eligibility of the would-be implicit members, which
needs no declaration), `LITERAL` ([basic.types.general]/10), `STRUCTURAL`
([temp.param]/7) follow the standard text with clang's interpretation where it
differs. `EMPTY` ([meta.unary.prop] `is_empty`, clang
`CXXRecordDecl::isEmpty`) ignores unnamed bit-fields and the subobjects of
zero size ([intro.object]/9): a `[[no_unique_address]]` field whose type is
an `EMPTY` class (not an array of one; clang `FieldDecl::isZeroSize`; WP-292,
test `templates/builtins/meta.unary.prop-is-empty-no-unique-address.cpp`).
Which implicit members have been declared is sema-side state
(`Sema.implicit_declared: FxHashMap<DeclId, u8>`), not a definition bit, so the
AST stays frozen.

## 3. Bases

### 3.1 Base-specifiers ([class.derived])
`act_on_base_specifiers(r, specs)`; `BaseSpecInfo { ty, loc, is_virtual,
access: Access, pack_expansion, attrs }`. Checks in order, each fatal for that
specifier only (the base is dropped, the class stays valid): dependent (WP-070
keeps it); not a class (`base specifier must name a class`); incomplete (`base
class has incomplete type` + note `forward declaration of 'T8'`); union base or
union with bases (`unions cannot be base classes` / `unions cannot have base
classes`); `final` (`base 'Fin' is marked 'final'` + note `'Fin' declared
here`); repeated direct base (`base class 'B' specified more than once as a
direct base class`). Valid specifiers become `kiln_ast::BaseSpec` records
(`DeclArena::base_specs`, `RecordData.bases`). Until WP-078 lands a `virtual`
base is `sema_unsupported("virtual base classes")`.

### 3.2 Member lookup across bases ([class.member.lookup])
`lookup.rs` gains `lookup_in_class(c, name) -> MemberLookup { decls, naming:
DeclId, paths, ambiguity }`: the declaration set of `c` itself (O(1) table hit)
or, if empty, the merge of each direct base's result per
[class.member.lookup]/5–7 (subobject sets; a static member, type or enumerator
found in several subobjects of the same type is not ambiguous; a virtual base
contributes one subobject; hiding by dominance). Ambiguity diagnostics: `member
'x' found in multiple base classes of different types` / `non-static member 'x'
found in multiple base class subobjects of type 'B'` with clang's notes and
path lines. Cost: O(classes in the hierarchy) on a miss; hits in `c` are O(1).

### 3.3 Derived-to-base conversion
`check_base_conversion(derived, base, loc, ctx) -> Option<ListId>` adds access
([class.access.base]/4) and ambiguity diagnostics to sema-conv's `find_base`:
`cannot cast 'Pr' to its private base class 'S'` (+ `declared private here` /
`constrained by private inheritance here`), `ambiguous conversion from derived
class 'DD' to base class 'B1':` + paths. Implicit conversion sequences form the
conversion regardless; the errors fire for the selected conversion only
(clang). Cast kinds: `DerivedToBase` for written and ICS conversions;
`UncheckedDerivedToBase` where sema converts a known-valid object it built
(implicit `this`, base initializers and assignments in implicit members);
`BaseToDerived` for `static_cast` downcasts (`cannot cast 'A *' to 'B *' via
virtual base 'A'`).

### 3.4 Virtual functions ([class.virtual], [class.abstract])
- **Overriding**, at declaration: for each direct base, `lookup_in_class` of
  the name; every virtual function with the same parameter-type-list, cv- and
  ref-qualifiers is overridden (also when hidden in between). The set goes to
  `overrides` (method → `ListId`); an overrider is virtual (FnSpec bit at
  creation); destructors override destructors.
- **Return types** ([class.virtual]/8): identical or covariant (class
  pointer/reference, base unambiguous and accessible, no more cv): `virtual
  function 'f' has a different return type ('long') than the function it
  overrides (which has return type 'int')`, `return type of virtual function
  'f' is not covariant with the return type of the function it overrides
  (...)`.
- **`override`/`final`** (FnSpec 18/19): `only virtual member functions can be
  marked 'override'` (or `'final'`), `'f' marked 'override' but does not
  override any member functions`, `declaration of 'f' overrides a 'final'
  function` + `overridden virtual function is here`;
  `-Winconsistent-missing-override`, `-Woverloaded-virtual` (`'HD::h' hides
  overloaded virtual function` + `hidden overloaded virtual function 'Base::q'
  declared here: type mismatch at 1st parameter ('int' vs 'long')`),
  `non-virtual member function marked 'override' hides virtual member
  function`.
  Which checks run follows clang's `CheckOverrideControl` (WP-322): a
  member function counts as virtual for them when declared `virtual` or
  when it overrides something (clang `isVirtual`), so a `static` member
  that overrides a base virtual keeps its `override`/`final` (no `only
  virtual ...` error, no `-Winconsistent-missing-override`; the static
  mismatch is the error, reported at `}`). A function with both
  specifiers that is not virtual gets the `'final'` error at `final` and
  the `'override'` one at `override`; a non-virtual pure function gets
  only the pure error. In a dependent class (dependent base or type) the
  checks wait for instantiation and report at the specifier's own column
  (`Sema::virt_spec_locs`, the instantiated declarator's
  `virt_override`/`virt_final`), with clang's `in instantiation of`
  note. Outside a class definition (`'override' specifier is not allowed
  outside a class definition`, also on a non-virtual or static member's
  out-of-line definition and a free function) and in a friend
  declaration (`'override' is invalid in friend declarations`) the
  specifier is rejected by the parser and no virtual check runs; a
  `requires`-clause after a virt-specifier is a parse error (`expected
  ';' at end of declaration list`) after the specifier's own check, and
  a virtual function's requires-clause is rejected again at
  instantiation. Diagnostic order matches clang (the corpus compares
  diagnostic sets; the order was checked by hand). Tests:
  `tests/conformance/class.virtual/override-control-static-template.cpp`,
  `override-control-requires-out-of-line.cpp`.
- **consteval** mismatch either way: `consteval function 'f' cannot override a
  non-consteval function` (and converse). Exception ([class.virtual]/18 as
  P2996R13 amends it; GCC 16.1 is the oracle, clang 22 has no rule): a
  consteval function may override a non-consteval one in a class of
  consteval-only type (a class with a `std::meta::info` member, `<meta>`'s
  `std::meta::exception::what`; WP-525,
  `class.virtual/consteval-override-consteval-only.cpp`).
- **Pure/abstract**: `= 0` only on virtuals (`'p' is not virtual and cannot be
  declared pure`); `ABSTRACT` iff some final overrider is pure. Abstract uses
  ([class.abstract]/3): variable (`variable type 'Abs' is an abstract class` +
  `unimplemented pure virtual method 'f' in 'Abs'`), `new` (`allocating an
  object of abstract class type 'Abs'`), field, parameter/return of a
  definition or call, temporary; uses inside the class are rechecked at `}`
  (clang `AbstractUsageInfo`).
- **Wrong kinds**: `'virtual' can only appear on non-static member functions`,
  `constructor cannot be declared 'virtual'`, `'virtual' cannot be specified on
  member function templates`.
- **Deleting destructor**: defining a virtual dtor looks up `operator delete`
  ([class.dtor]/16) into cold map `dtor_delete`, access-checked at the
  destructor (WP-457); for a destroying `operator delete` the cold map
  `dtor_delete_this` holds `this` converted to its first parameter, the
  deleting destructor's argument (spec/codegen.md §7u).
- **Key function** (Itanium §5.2.3): first non-pure virtual function not inline
  and not defined in the class, computed at `}` (cold map); codegen re-checks
  at TU end.

### 3.5 Subobjects, final overriders, vtables
**Subobject numbering.** For a most-derived class `D`, `SubobjectId(u16)`
numbers the base-subobject graph in preorder: 0 = `D`, then each direct
non-virtual base's subtree in declaration order, then each virtual base (once)
in Itanium inheritance-graph order with its non-virtual subtree. This is the
shared currency between sema, Oven and codegen (codegen maps a `SubobjectId` to
an offset via `RecordLayout`; the Oven maps its pointer path to it).

**Final overriders** (`virtual.rs`, WP-078a), computed on demand per
polymorphic class and cached insert-only in `Ast.virtuals`:
```
pub struct ClassVirtuals {
    pub subobjects: Vec<Subobject>,          // { class, parent: SubobjectId, base_index: u16, is_virtual }
    pub overriders: Vec<(SubobjectId, DeclId, DeclId, SubobjectId)>, // (where, method) -> (overrider, its subobject), sorted
}
```
[class.virtual]/2–3: the unique final overrider per (subobject, virtual
method); none unique → `virtual function 'f' has more than one final overrider
in 'E'` (error at `}`). Dominance through virtual bases is handled by the
subobject graph (the overrider's subobject need not be on the path to the
subobject).

Complexity (`class/virtual/graph.rs`, WP-414). The final overriders of
(x, m) are the maximal subobjects, in the containment DAG above x, whose
class declares an overrider of m. Each method has *override roots* (the
methods it transitively overrides that override nothing; memoised). The
pairs are grouped by the first root m0 of m and one postorder walk per
root computes, for every subobject, the maximal subobjects above it
declaring an m0-overrider; when all of them override m itself (the usual
case: an m-overrider is an m0-overrider) they are m's finals, otherwise
that pair falls back to a direct walk. Total work is linear in the
(subobject, virtual function) pairs times the containment edges, not
quadratic in the subobjects per pair. The subobject graph finds the
parent of each virtual base from an incremental class → subobject map.
The final overriders a used vtable marks referenced are cached per class
(`Sema.vtable_finals`), so `define_used_vtables` recursing through the
bases does not recompute them.

**Itanium vtables** (`kiln_abi::vtable`, WP-078b), cached in `Ast.vtables`:
```
pub struct VTableLayout {
    pub components: Vec<VTableComponent>,    // VCallOffset(i64) | VBaseOffset(i64) | OffsetToTop(i64) | Rtti(DeclId)
                                             // | Func(DeclId) | CompleteDtor(DeclId) | DeletingDtor(DeclId) | UnusedFunc(DeclId)
    pub address_points: Vec<(SubobjectId, u32)>,
    pub thunks: Vec<(u32, Thunk)>,           // component index -> { this: (non_virtual i64, vcall_offset_offset i64), ret: ... }
    pub vbase_offset_offsets: Vec<(DeclId, i64)>,
    pub method_indices: Vec<(DeclId, u32)>,
}
```
plus `vtt(record) -> Vec<VttComponent>` and construction vtables for classes
with virtual bases (Itanium §2.6). The current
`kiln_oven::VTableLayout { slots }` is replaced (§8.4).

Landed (WP-078b): the struct also carries
- `vtable_starts: Vec<u32>`: the first component of each vtable in the
  group;
- `method_thunks: Vec<(DeclId, Thunk)>`: the thunks to emit per
  overrider, as clang's `Thunks`.

`Thunk` gains `method: Option<DeclId>` and `this_class: Option<DeclId>`
(clang `ThunkInfo::{Method, ThisType}`) and `is_empty()`. The
`address_points` name subobjects of the group's most-derived class and
point after the RTTI slot. The `method_indices` include the methods of
the primary-base chain. Full contract and signatures: abi.md §1, §8.5.

**Vtable uses** ([basic.def.odr]/8; clang `MarkVTableUsed`,
`DefineUsedVTables`, `MarkVirtualMembersReferenced`; `virtual/used.rs`,
WP-137). A class's vtable is used, once per class, by:
- a constructor definition: a user-written one at the end of its
  ctor-initializer completion (located at the constructor name), an
  implicit one at the use site that defined it;
- a destructor definition, written or implicit, at the same locations;
- the definition of the key function (outside a template), at its name.

Only a dynamic, non-dependent class outside an unevaluated context counts.
A local class is marked at once; any other class is queued in
`Sema.vtable_uses`. At the end of the TU, `end_of_tu_instantiations`
alternates `define_used_vtables` with draining the pending instantiations
until both are empty, as clang does. `define_used_vtables` skips a class
whose key function is declared but not defined in this TU. Otherwise it
marks each non-pure final overrider referenced (in method order, which
instantiates it). It then recurses into each direct base that has virtual
bases, for the VTT. Not modelled: explicit-instantiation-declaration
suppression of the vtable.

## 4. Special member functions
### 4.1 Implicit declaration
| member | declared implicitly when | parameter |
|---|---|---|
| default ctor | no user-declared ctor ([class.default.ctor]/1) | — |
| copy ctor | no user-declared copy ctor ([class.copy.ctor]/6); deleted if a move ctor/assign is user-declared | `const X&` iff `IMPLICIT_COPY_CTOR_CONST_PARAM`, else `X&` |
| move ctor | none of copy ctor, copy assign, move assign, dtor user-declared ([class.copy.ctor]/8) | `X&&` |
| copy assign | no user-declared copy assign ([class.copy.assign]/2); deleted as for copy ctor | `const X&` / `X&`, returns `X&` |
| move assign | as move ctor ([class.copy.assign]/4) | `X&&`, returns `X&` |
| destructor | no user-declared destructor ([class.dtor]/2) | — |

An implicit member is a public `CXXConstructor`/`CXXMethod`/`CXXDestructor`
with `DeclFlags::IMPLICIT`, `Defaulted::Implicit`, inline, `constexpr` per
§4.6, `trivial` and `deleted` known at creation (§4.2–4.3), one unnamed
`ParmVar` for copy/move, and `ExcSpec::Deferred(self)` (dump
`noexcept-unevaluated`) resolved when the member is defined or its exception
specification is needed ([except.spec]/13).

**Laziness** (clang `AddImplicitlyDeclaredMembersToClass`). At `}`, of the
members still to be declared implicitly: the default ctor if
`HAS_INHERITED_CTOR`; the copy ctor if `NEEDS_OVL_COPY_CTOR`, a move
operation is user-declared, or `HAS_INHERITED_CTOR`; the move ctor if
`NEEDS_OVL_MOVE_CTOR` or `HAS_INHERITED_CTOR`; the copy assignment if
`DYNAMIC`, `NEEDS_OVL_COPY_ASSIGN`, a move operation is user-declared, or
`HAS_INHERITED_ASSIGN`; the move assignment if `DYNAMIC`,
`NEEDS_OVL_MOVE_ASSIGN` or `HAS_INHERITED_ASSIGN`; the dtor if `DYNAMIC` or
`NEEDS_OVL_DTOR`. Otherwise on first lookup of the constructor name,
`operator=` or the destructor in that class, or when another class's
overload resolution selects among them (a `{ ~P1(); }` alone declares
nothing, but `{ P1 m; }` declares P1's copy members); such members are
appended to the member list (§8.2), as clang. Observed clang 19 (golden
`ast/class.implicit/eager.cpp`):

| class | eagerly declared |
|---|---|
| `{ int a; }`, `{ ~D1(); }`, `{ P2(const P2&); }`, `{ P3(); }`, `{ mutable int m; }`, `{ CA& operator=(const CA&); }` | none |
| `{ virtual void f(); }` and its derived classes | copy assign, move assign, dtor |
| `{ P1 m; }` (member with user dtor) | copy ctor, move ctor, move assign, dtor |
| `{ MC(MC&&); }`, `{ MA& operator=(MA&&); }` | copy ctor, copy assign (both deleted) |

### 4.2 Deletedness
`should_delete(class, kind) -> Option<DeleteReason>` implements
[class.default.ctor]/2, [class.copy.ctor]/10, [class.copy.assign]/7,
[class.dtor]/7, [class.union.general]/3: per potentially constructed subobject,
the corresponding special member selected by overload resolution (S3O's
`select_special_member(class, kind, quals)`) is ambiguous, deleted or
inaccessible from the defaulted member; variant members with non-trivial
members; reference/const members without NSDMI (default ctor); reference/const
members (copy/move assign); a user-declared move operation (copy members); for
a defaulted dtor, an inaccessible or deleted `operator delete` of a virtual
dtor (WP-457: an implicit or defaulted virtual dtor whose deallocation
function — usual or destroying, looked up as a delete-expression in the
class would — is not found, is ambiguous, or is inaccessible from the class
is deleted, note `virtual destructor requires an unambiguous, accessible
'operator delete'` at the class; `virtual_dtor_delete_usable`). Until WP-068a lands, selection uses the unique non-template candidate with
a matching parameter form (Q7). The `DeleteReason` drives the notes: `default
constructor of 'Y' is implicitly deleted because base class 'S' has no default
constructor`, `... because field 'r' of reference type 'int &' would not be
initialized`, `copy assignment operator is implicitly deleted because 'T10' has
a user-declared move constructor`. Uses: `call to implicitly-deleted default
constructor of 'Y'`, `object of type 'T10' cannot be assigned because its copy
assignment operator is implicitly deleted`, `call to deleted constructor of
'NC'` + note `'NC' has been explicitly marked deleted here`. An explicitly
defaulted member that would be deleted: deleted, plus `explicitly defaulted
default constructor is implicitly deleted` [-Wdefaulted-function-deleted] +
note `replace 'default' with 'delete'` (or an error when the declared type
differs, [dcl.fct.def.default]/2).

### 4.3 Triviality ([class.default.ctor]/3, [class.copy.ctor]/11, [class.copy.assign]/9, [class.dtor]/8)
Not user-provided, no virtual functions/bases (ctor, copy/move, assign), every
direct base and non-static class member's selected corresponding member
trivial, no NSDMI (default ctor), not a variant-member conflict. Computed at
`}` into `TRIVIAL_*`; the implicit member gets `FnSpec` trivial at creation
(from `TRIVIAL_*`, or, for a copy/move member whose class has `NEEDS_OVL_*`,
by selecting each direct base's and class field's member as clang
`SpecialMemberIsTrivial`: `{ P1 m; }` has a trivial move ctor because P1's
copy ctor is selected). That selection also sets or clears the class's own
`TRIVIAL_*` bit (clang `addedEligibleSpecialMember`; the member is declared
eagerly at `}`, and while the class is finishing it also updates
`TRIVIALLY_COPYABLE`), so `struct S : E {}` and `{ E e; }` see `E`'s move
ctor as trivial (tests/conformance/class.copy.ctor/trivial-through-overload-resolution.cpp).
An `= default` on first declaration gets it at `}` (`mark_function`, already
`LATE`), by the same selection when the class has `NEEDS_OVL_*` for it
(WP-292: `{ M m; S(S&&) = default; }` with `M` declaring only a defaulted
copy is trivially move-constructible). Eligibility ([special]/6) of
constrained special members is WP-073's;
for non-template classes every non-deleted declared special member is eligible.

### 4.4 `= default`, `= delete`, user-provided
`act_on_defaulted(f, loc)` / `act_on_deleted(f, loc, msg)` (C++26 message in
the cold map). Defaultable: special members and comparison operators
(defaulted comparisons, [class.compare], are §4.12's); else `only special
member functions and comparison operators may be defaulted`. An `= default`
written in the class body is a definition there and so implicitly inline
([class.mfct]/1; clang `setImplicitlyInline`; the dump prints
`implicit-inline`); one written out of class is not. Explicitly defaulted on its first
declaration → not user-provided; out of line → user-provided, defined there
(`defaulted_impl` → `define_implicit_special` on the out-of-line declaration
once `check_out_of_line_defaulted` passes; clang `SetDeclDefaulted` →
`DefineDefaultedFunction`), so codegen emits it and the Oven evaluates it. A
member of a class template whose pattern is defaulted out of line is defined
when the member's definition is instantiated
(`instantiate_out_of_line_defaulted`, without the `= default` checks, as
clang), its context note "in defaulted ... first required here" at the
pattern's definition. The member is located at that definition from then
on (sema-templates.md §4.6, WP-428), so an error inside the instantiated
definition, and its note chain, are clang's. A defaulted copy or move
assignment operator is defined at the end of its declaration (clang
`DefineImplicitCopyAssignment` `getEndLoc()`, `declarator_end`,
spec/ast.md): the declarator's closing `)` out of line, the last character
of `default` in the class, the class for an implicit one; an instantiation
of a member's `operator=` is "requested here" there. An out-of-line
definition skips the deletion checks, so a member or base whose selected
`operator=` is deleted is `attempt to use a deleted function` with the
"explicitly marked deleted here" note ([dcl.fct.def.delete]/2). Tests:
`tests/conformance/dcl.fct.def.default/out-of-line-defined-run.cpp`,
`constexpr-out-of-line-cxx20.cpp`, `instantiated-out-of-line-loc.cpp`,
`assign-instantiation-point.cpp`. A
deleted function must be deleted on its first declaration. Signature checks of
[dcl.fct.def.default]/2–3 (`the parameter for an explicitly-defaulted copy
constructor is const, but a member or base requires it to be non-const`, return
type of assignment, `explicitly-defaulted copy assignment operator must return
'X &'`).

### 4.5 Implicit definition
An implicit or defaulted-on-first-declaration member is defined when odr-used
(`mark_function_referenced` → `define_implicit_*`, clang's `DefineImplicitX`),
when its exception specification is needed (spec only), or when the Oven asks
for its body (`SemaBridge::ensure_body`). Bodies, exactly clang's (verified by
`implicit`-mode goldens):
- **ctors**: the completed ctor-initializer list (§4.9) and an empty
  `CompoundStmt`. Copy/move: base `CXXConstructExpr` from the unnamed parameter
  cast `UncheckedDerivedToBase` (move: from an xvalue of it, shape per
  clang's `implicit` dump); class field
  `CXXConstructExpr` from `MemberExpr .m` on the parameter; scalar field
  `LValueToRValue(MemberExpr)`; array field
  `ArrayInitLoopExpr`/`ArrayInitIndexExpr`. Default: base/field
  `CXXConstructExpr` (default-initialization), `CXXDefaultInitExpr` for an
  NSDMI, nothing for a trivially default-initialized scalar.
- **assignment**: per base `this->B::operator=(static_cast<const B&>(o))` (a
  `CXXMemberCallExpr`, or `CXXOperatorCallExpr` per clang), per field `this->m
  = o.m` (builtin `BinaryOperator` for scalars, call for classes,
  `__builtin_memcpy` for trivially copyable arrays, a loop for others), then
  `return *this;`.
- **dtor**: an empty `CompoundStmt`; sema marks every subobject destructor
  referenced and checks it (clang `MarkBaseAndMemberDestructorsReferenced`).

Errors during an implicit definition carry the note `in implicit copy
constructor for 'T' first required here` (and the other member names).
`DeclFlags::USED` is set on the member (dump `used`).

### 4.6 `constexpr`
C++23 and later (P2448): an implicit or defaulted constructor, assignment or
destructor is `constexpr` unless the class has a virtual base (dump `constexpr`
on all implicit members of non-virtual-base classes, absent on
`VB(...)`/`~VB`). C++20: the [dcl.constexpr]/3–4 per-subobject rule (`NN()` is
not `constexpr` when a member's selected ctor is not; a destructor only when
every subobject's is, clang `defaultedDestructorIsConstexpr`; none before
C++20). `HAS_CONSTEXPR_CTOR` and
`LITERAL` are computed from these. `DEFAULTED_DEFAULT_CTOR_CONSTEXPR` records
the default ctor's answer before it is declared.

WP-400 models clang's `DefaultedDefaultConstructorIsConstexpr` and
`defaultedSpecialMemberIsConstexpr` (`defaulted_is_constexpr`):
- From C++23 every defaulted special member of a class without a virtual
  base is constexpr (C++26: with one too, P3533); a non-constexpr member or
  base constructor fails only when an evaluation reaches it, with the
  defaulted constructor in the call stack.
- C++23 with a *direct* virtual base (indirect ones do not count; copy and
  move constructors are never constexpr, assignments always are): the
  default constructor and destructor follow clang's running flag
  (`vbase_flag_cxx23`). It starts true; over the bases in declaration
  order a virtual base clears it and a non-virtual base without a
  constexpr default constructor (destructor) sets it; then over the
  fields a class-type member (an array's element type; references and
  anonymous-union members skipped; for the constructor, a member with a
  default member initializer skipped too) without one sets it. So
  `A : virtual V, NB` may declare `constexpr A() = default;` but
  `A : NB, virtual V` may not. A destructor so found constexpr is
  recorded in `SpecialState::vbase_dtor_constexpr` (dump `constexpr`);
  the class stays non-literal ([basic.types.general]/10, `LITERAL`
  unaffected). Test:
  `tests/conformance/class.default.ctor/constexpr-defaulted-vbase-order-cxx23.cpp`.
- Before C++23 the per-subobject rule holds: each base's and each member's
  selected constructor (default, copy or move as the member requires) or
  assignment must be constexpr, a member without an initializer and a union
  are fine (P1331), and a virtual base makes it non-constexpr.
- An explicitly `constexpr`/`consteval` defaulted member that the rule says
  is not constexpr is an error at the specifier before C++23: "defaulted
  definition of {default constructor|copy constructor|...} cannot be marked
  {constexpr|consteval} before C++23"; with a virtual base it is the
  [dcl.constexpr]/4 error instead (§4.10). A destructor is never consteval.
- The pending defaulted members of a class are checked destructor first,
  then in declaration order (clang `CheckCompletedCXXClass`).
- A defaulted comparison declared `constexpr`/`consteval` before C++23
  whose synthesized body would call a non-constexpr comparison is an error
  at the specifier ("defaulted definition of {equality|three-way|relational}
  comparison operator cannot be declared constexpr because it invokes a
  non-constexpr comparison function", clang 19 with a trailing space: `_19`
  row), with clang's `ExplainConstexpr` notes for the first such callee
  ("non-constexpr comparison function would be used to compare {member
  X|base class X}" at the subobject, omitted for a secondary comparison of
  the complete object, then "... declared here"). A defaulted `<=>` checks
  its implicit `operator==` too, reported at the `<=>` ("because for which
  the corresponding implicit 'operator=='  invokes ..."). A class template
  specialization is checked when instantiated, at the pattern's specifier.
- Tests: `tests/conformance/class.default.ctor/constexpr-defaulted-cxx{20,23}.cpp`,
  `tests/conformance/dcl.fct.def.default/consteval-special-cxx{20,23}.cpp`,
  `tests/conformance/dcl.fct.def.default/cmp-constexpr-mismatch{,-cxx23}.cpp`.
- An implicit or defaulted constructor (in the class or out of line) that
  can never be constant is not diagnosed at its definition, even under
  `-Winvalid-constexpr` (clang 19 and 22 run `isPotentialConstantExpr`
  only on written bodies); a use in a constant expression gets the
  default member initializer's note and "in call to 'X()'"
  (`dcl.fct.def.default/constexpr-out-of-line-cxx20.cpp`). Kiln implements the check for written bodies only for a
  body overflow in a function without parameters ("constexpr
  {function|constructor} never produces a constant expression", an
  error by default before C++23; spec/sema.md §5.28 "Never constant"). Open: clang 14-17's
  acceptance of a vbase class with an invalid constexpr constructor as
  literal (not mirrored; tests pin C++20 and later).

### 4.7 Constructors, destructors, conversion functions (declarations)
Constructor: no return type (`constructor cannot have a return type`), not
`static`/`virtual`/cv-/ref-qualified, copy ctor by reference (`copy constructor
must pass its first argument by reference`), `explicit` / `explicit(bool)`
(cold map). A non-explicit constructor is converting ([class.conv.ctor]).
The operand of `explicit(expr)` is a contextually converted constant
expression of type `bool` ([dcl.fct.spec]/4; `Sema::resolve_explicit`, the
`Sema::bool_cce` shared with the *noexcept-specifier*, WP-275): a class type
converts through a constexpr conversion function; a type with no conversion,
a disallowed conversion, a narrowing and a non-constant operand get clang's
converted-constant-expression errors at the operand (`explicit specifier
argument ...`), after which the declaration is not explicit. A value-dependent
operand waits for instantiation; an instantiated member whose operand is then
invalid is dropped (clang `InstantiateExplicitSpecifier`).
Destructor: name matches the class, no parameters (`destructor cannot have any
parameters`), no return type; implicitly `noexcept` unless a subobject's isn't
([except.spec]/9). Conversion function ([class.conv.fct]): no return type
(`conversion function cannot have a return type`), no parameters, `DeclName`
tag 4 on the canonical target; conversion to the class itself, a base or `void`
warns [-Wclass-conversion]. `explicit` elsewhere: `'explicit' can only be
applied to a constructor or conversion function`. Selection (converting ctors
and conversion functions in user-defined conversion sequences) is S3O's.

**Explicit object parameters** ([class.conv.fct]/1, [dcl.fct]/6, P0847;
WP-459). A conversion function may declare an explicit object parameter
(`operator int(this const S&)`): "no parameters" counts only the non-object
ones (clang `CheckConversionDeclarator`), and the declaration gets the
explicit-object flag through the same `explicit_object_member` check as any
member, which runs before the conversion function's own checks (so `static`
and a non-first `this` report clang's explicit-object errors, the latter
before `conversion function cannot have any parameters`). `operator=` takes
one the same way. A constructor or destructor cannot (`an explicit object
parameter cannot appear in a {constructor|destructor}`, at `this`; the
declaration is invalid). In a user-defined conversion sequence the implied
object argument initializes the explicit object parameter by copy-
initialization with user-defined conversions suppressed (clang
`TryObjectArgumentInitialization`), so value-category and cv overloads on the
object parameter pick as for any call; a conversion function template also
deduces its object parameter from the implied object argument (`this auto&&`,
`this Self&&`; `deduce_conversion` gets the object as the one call argument).
The call is built as an explicit-object call (object as first argument, no
temporary materialized for it). Before C++23 every explicit object parameter
is an error (`explicit object parameters are incompatible with C++ standards
before C++2b`, parser, at `this`; parsing continues as C++23).

#### 4.7.1 Prospective destructors ([class.dtor]/4; WP-289)
A class may declare several *prospective* destructors, distinguished by
their trailing *requires-clause*s. Every one is an ordinary member in lookup
(the parser records them all). At the class's closing `}` — of the template
pattern never, of each instantiation once (`act_on_finish_record_definition`,
`Sema::select_destructor`, before the special-member bits) — when at least
one non-`INVALID` prospective destructor has a trailing requires-clause,
the destructor is selected by overload resolution with an empty argument
list among them (clang `ComputeSelectedDestructor`): constraints are checked
([over.match.viable]/3) and the more constrained one wins
([over.match.best]/2.6, [temp.constr.order]). The outcome:
- **one best**: it is the destructor; every other prospective destructor is
  marked `FnSpec::not_selected` (ast.md; clang `isIneligibleOrNotSelected`).
- **ambiguous**: `destructor of class 'T' is ambiguous` at the class name,
  then a `candidate function` note per tied candidate.
- **none viable**: `no viable destructor found for class 'T'` at the class
  name, then `candidate function not viable: constraints not satisfied` per
  candidate with its `because ...` satisfaction notes.
After either error the first declared prospective destructor is used and
the others are `not_selected`; the class is not made `INVALID` (as clang).
A class without constrained destructors keeps today's single-destructor
path. `Ast::destructor` returns the first destructor found that is not
`not_selected`; the special-member scan (`scan_declared`, §4.1) skips
`not_selected` ones, so triviality ([class.dtor]/8) and the implicit
`noexcept` follow the selected destructor only. A destructor has no
explicit object parameter ([class.dtor]/1), so constraints are the only
distinction.

A requires-clause on a **non-templated** destructor (or any non-templated
function) is an error, `non-templated function cannot have a requires
clause` at the clause's first token ([temp.constr.decl]/1, [dcl.decl]/4),
but, as in clang, the clause is kept and checked with no template
arguments wherever constraints are checked (`ConceptState::
untemplated_requires`, `check_function_constraints`): destructor
selection then reports `no viable destructor found` (or picks the
satisfied one) exactly as for a template, and calls and references
report `invalid reference to function 'f': constraints not satisfied`.
For an explicit instantiation of a class template specialization that
is first named by it (clang `ActOnExplicitInstantiation`'s
`setLocation(TemplateNameLoc)`), the specialization's location moves to
the template name in the explicit instantiation before the class is
instantiated (`AstBuilder::relocate_spec`), so a selection error is
reported there. `tests/conformance/class.dtor/prospective-destructor-non-template.cpp`.

A `virtual` function with a trailing requires-clause — `virtual` as
written, an overrider, or an out-of-class definition of one — is
diagnosed `virtual function cannot have a requires clause` at the
clause's first token ([class.virtual]/6, clang `CheckFunctionDeclaration`
`err_constrained_virtual_method`), once per instantiation
(`per_instantiation`); a requires-clause the parser already rejected
(`is_err_expr`) is not diagnosed again. `virtual` on a member function
template, including an abbreviated one (`h(auto)`), is diagnosed
`'virtual' cannot be specified on member function templates` at the
keyword with a removal fix-it ([temp.mem]/3, clang
`err_virtual_member_function_template`), and the function is then not
virtual. `tests/conformance/class.virtual/requires-clause.cpp`.

### 4.8 Destruction of variables
Sema does not insert destructor calls for named objects: a variable, parameter
or `LifetimeExtendedTemporary` whose type is destructed ([basic.life], clang
`isDestructedType`) is destroyed by the Oven/codegen at scope exit in reverse
declaration order. Sema's obligation when such a variable is declared: the
destructor is accessible (`variable of type 'X' has private destructor`) and
not deleted (`attempt to use a deleted function`), and is marked referenced
(implicit definition). An array variable's element destructor is marked
referenced without a check (clang's `FinalizeVarWithDestructor` skips
arrays); it is checked where each element is initialized instead.

**Element destructors** (WP-227, clang `checkDestructorReference`;
`init_dtor.rs` with hooks in `init.rs` and `class/construct.rs`): each
subobject an initialization creates must be destructible, so an
inaccessible destructor is `temporary of type 'X' has private destructor`
and a deleted one `attempt to use a deleted function` (+ `'~X' has been
explicitly marked deleted here`), at clang's locations:
- an array's element type once per array (its own list's `}`, or the
  outermost list's `}` under brace elision), before its elements; also
  for a default- or constructor-initialized array (the declarator);
- an aggregate's field before its initializer (at its begin), or after
  it when designated (at the designator); a base after its initializer;
  the fields left implicit, after the last element (all of them once any
  designator appears; every field of an empty union list), at the `}`;
- a converted element that is destroyed but not bound to a temporary
  (a user-defined conversion initializing a variable, member or element),
  at the converted expression.
A deleted destructor stops the list (the rest is not checked, clang's
`hadError`). Deviations: clang reports access errors at the end of the
declaration and deleted ones at once, so their relative order differs in
a declaration that has both; subobjects reached through a designator's
path (`.a.b = ...`) are not checked; clang's same-location repeats are
deduplicated (diag.md §7.4).

### 4.9 Ctor-initializers ([class.base.init])
`act_on_mem_initializers(ctor, &[MemInitInfo], colon_loc)`, `MemInitInfo { id:
Ident | TypeId, loc, init, pack_expansion }`. A mem-initializer-id names a
non-static data member (anonymous-union members via `IndirectField`), else a
direct or virtual base, or the class itself (delegating). An identifier that
is not a field is looked up in the class first, where it may be a member typedef
or an injected-class-name, including one inherited from a base
([class.qual]/2). It is then classified in the enclosing scopes. There, a class
template's injected-class-name is the injected type ([temp.local]/1). This is
also true when an instantiated constructor re-resolves its ids, whose scope
chain lacks the class scope. Errors: `member
initializer 'y' does not name a non-static data member or base class`,
`multiple initializations given for non-static member 'x'` (`... for base
'B'`), `an initializer for a delegating constructor must appear alone`,
`initializing multiple members of union`.

**In a template** (WP-223; clang `BuildMemInitializer` in a dependent
context). The list is kept as written and substituted per instantiation, but
an unqualified id is checked when the definition is parsed: it must name a
field of the class or a type found by that lookup (member typedef,
injected-class-name, a type in an enclosing scope, a template parameter). A
dependent base is not searched ([temp.dep.general]/3), so `B(1)` for base
`B<T>` gets the `does not name` error at the definition, and the
constructor's instantiations add no implicit initializers. A qualified id
whose qualifier can name an unknown specialization (`T::Self(2)`, or the
current instantiation with a dependent base, `D::B(1)` / `D<T>::B(1)`) is a
`DependentNameType` (`Sema::act_unknown_specialization_type`). It is looked
up in the instantiation (`no type named 'Foo' in 'D7<int>'`). The same
function makes `typename D::X` dependent for such a qualifier
([temp.res.general]/5). A qualified id naming no type in a non-dependent
qualifier is still `qualified member initializers` unsupported. A
mem-initializer that fails in an instantiation (its substitution, or its
initialization: `decltype(t)(t)` naming a reference type) makes the
constructor specialization invalid, as clang's `InstantiateMemInitializers`
does: it is not a candidate afterwards and a constant evaluation already
calling it fails with no evaluation note (WP-374; test
`class.base.init/failed-initializer-constexpr.cpp`). A failure in the
definition-time check above does not.

**Completion** (clang `SetCtorInitializers`): stored in initialization order
([class.base.init]/13): virtual bases (inheritance-graph order, complete-object
ctor only), direct bases, fields. An unwritten subobject gets
`CXXDefaultInitExpr` (NSDMI), a default-initializing `CXXConstructExpr`
(construction kind non-virtual base / virtual base / complete), nothing
(trivially default-initialized scalar), or an error: `constructor for 'I' must
explicitly initialize the reference member 'r'` / `... the const member 'c'` (+
`declared here`), `constructor for 'UU' must explicitly initialize the member
'u' which does not have a default constructor` (+ `member is declared here`,
`'U' declared here`), `... the base class 'B' which does not have a default
constructor`; every such base and member is reported, not only the
first (WP-409, `class.base.init/every-base-and-member-without-default-ctor.cpp`).
Each written initializer is a full-expression; so is each implicit one
(a default-initializing construction of a member with no NSDMI), which is
an immediate-escalation site for an implicitly defined constructor
([expr.const]/20; WP-158: `struct E { B b; }` with a consteval `B()`
escalates `E()`, the reason naming the member). In a template the list is
kept unbuilt until instantiation (`inst.pattern_mem_inits`), but the
non-dependent immediate invocations of each written initializer are
finished at the definition (`finish_immediate_parts`), and checked again
in each instantiation (sema.md §8 "Templates"; WP-227). `reference member
'r' binds to a temporary object whose lifetime would be shorter than the
lifetime of the constructed object` + `reference member declared here`.
Warnings: `field 'b' will be initialized after field 'a'` [-Wreorder-ctor],
`field 'b' is uninitialized when used here` [-Wuninitialized] (the value
walk reads a `?:` through `operand` + `cond_arms`: condition and both arms are
uses; `tests/conformance/class.base.init/uninitialized-conditional.cpp`,
WP-556); delegation
cycles at TU end: `constructor for 'S' creates a delegation cycle`
[-Wdelegating-ctor-cycles].

**Storage**: `CtorInit { target: u32 (field DeclId or base TypeId), kind: u8
(Base | VirtualBase | Field | IndirectField | Delegating), written_index: u8 (0
= implicit), init: ExprId, loc }` (16 B) in arena `Ast.ctor_inits`, cold map
ctor → `ListId`, written once with the body. Dump `CXXCtorInitializer 'S'` /
`CXXCtorInitializer Field 'm' 'int'`; JSON `baseInit` / `anyInit` /
`delegatingInit`.

**Default member initializers of a class template** (WP-223, WP-238).
As clang, an NSDMI of a specialization is instantiated when first used
(`in instantiation of default member initializer 'S<P>::b' requested
here`, at the constructor; spec/sema-templates.md §4.9) and a failed one
makes its field invalid, so the constructor reports nothing more. The
implicit constructor locations are the constructor's (`complete_ctor_inits`;
for an instantiated constructor defined out of line, its pattern's
definition). An inheriting constructor marks the inherited base
constructors used at the use, not at the using-declaration (clang
`DefineInheritingConstructor` `CurrentLocation`).

### 4.10 Inheriting constructors ([namespace.udecl]/3, [class.inhctor.init])
**Landed (WP-114).** Modules `kiln_sema/src/using.rs` (class-scope
using-declarators), `class/inhctor.rs` (the inheriting constructor),
`overload/{ics,best,diag}.rs` (candidates).
- **Declaration** ([namespace.udecl]/3, [class.qual]/2). A class-scope
  using-declarator's qualifier must name a direct or indirect base
  (`using declaration refers into 'X::', which is not a base class of
  'D'`). `using B::B;` (the last component names the qualifier's class:
  class, typedef or template parameter name) is a `Using` named by the
  derived class's constructor name (dump `UsingDecl B::D`) with one
  `ConstructorUsingShadow` per constructor of `B`, its own and the ones `B`
  itself inherits (`inheritable_ctors`); `DefinitionBits::HAS_INHERITED_CTOR`
  marks the class. A derived constructor with the same parameter-type-list
  (and template head) hides a shadow ([namespace.udecl]/14): an earlier one
  prevents it, a later one (including the implicit ones declared at `}`)
  is recorded by `hide_inherited_ctors` in `Decls::hidden_shadow`, and a
  hidden shadow is dropped from lookup and from the dump (clang
  `HideUsingShadowDecl`). An inherited default constructor makes the class
  declare its own implicit default constructor even with user-declared ones
  (clang `HasInheritedDefaultConstructor`).
- **Dependent bases** ([temp.dep.type], [temp.inst]/3). With a dependent
  qualifier the declarator is an `UnresolvedUsingValue` (a constructor name
  is the canonical nominated type: `Base<type-parameter-0-0>`, clang's
  dump); class instantiation replays it in the instantiated class
  (`member_using` in `templ/class.rs`), which then creates the shadows.
  libc++'s `using __base::__base;` takes this path.
- **Candidates** ([over.match.funcs.general]/8, [over.match.best]/2.7).
  Constructor lookup adds each visible shadow's target (templates are
  deduced as usual), accessed as the using-declaration
  ([namespace.udecl]/19). A copy/move constructor of a base whose first
  parameter's referred type is reference-related to the argument's class
  is not viable when the argument is (derived from) that base
  (`CandidateFailure::InhCtorSlice`, [over.match.funcs.general]/8; checked
  before arity and conversions as clang, so it overrides those failures;
  no note for non-template copy/move constructors, as clang
  `isCopyOrMoveConstructor`). `has_init_list_ctor` and the
  [over.match.list] phase-1 candidates see inherited initializer-list
  constructors (`ctors_and_inherited`, WP-146). In `better_viable`, a constructor
  of the derived class beats an inherited one when neither is otherwise
  better (/2.7; clang does not check the same-parameter-types condition and
  neither does kiln).
- **The inheriting constructor** (clang `findInheritingConstructor`). A
  selected shadow target materializes, once per (class, base constructor),
  an implicit inline `CXXConstructor` of the derived class named as the base
  constructor, with its parameter types and *explicit-specifier*,
  `ast.shadow_constructed`/`shadow_base_class` naming the shadow chain;
  `constexpr` when the base constructor is and a defaulted default
  constructor of the class could be; deleted when the base constructor is
  or when a defaulted default constructor would be ([class.inhctor.init]/1,
  [class.default.ctor]/2; clang `ShouldDeleteSpecialMember` with an
  `InheritedConstructorInfo`: the inherited bases through their inherited
  constructors, every other subobject's default constructor and
  destructor for deletion and access, uninitialized reference and const
  members; WP-146). A use is clang's "constructor inherited by 'D' from
  base class 'B' is implicitly deleted" with the "constructor inherited
  by 'D' is implicitly deleted because ..." note; selection reports it as
  deleted (`__is_constructible` is false). A second `using B::B;` naming
  the same base in one class is "redeclaration of using declaration"
  with "previous using declaration" and no shadows
  (`previous_inheriting_using`, WP-146). Defined on first odr-use
  (`define_inheriting_ctor`): `CXXInheritedCtorInitExpr` for each inherited
  base (`inherited_bases`: the nominated base and, through virtual bases,
  the base whose constructor is ultimately inherited, clang
  `InheritedConstructorInfo`), every other subobject as a defaulted default
  constructor would. The exception specification is clang's
  `ComputeInheritingCtorExceptionSpec`. An inherited constexpr
  constructor or constructor template that is not a copy or move
  constructor counts as a constexpr constructor of the derived class for
  the literal-type test ([basic.types.general]/10.5.2, clang `addedMember`
  on a `ConstructorUsingShadowDecl`; `inherited_constexpr_ctor`, WP-241).
  Its access is the base constructor's, named in the base class
  ([namespace.udecl]/19), for a complete object and for a base or member
  subobject a mem-initializer initializes through it alike (clang
  `CheckConstructorAccess` on the base constructor): a friend of the
  base may use a private one (WP-525,
  `namespace.udecl/inherited-ctor-subobject-access.cpp`). For a complete
  object the [class.protected] object class is the derived class being
  constructed (clang: the `ConstructorUsingShadowDecl`'s class), so a
  member of that class may call an inherited protected constructor
  (`Sema::special_member_access_on`;
  `namespace.udecl/inherited-protected-ctor-object.cpp`, WP-680).
- **Virtual bases** ([basic.types.general]/10). A class with a virtual
  base is not a literal type: a `constexpr` variable of it (or of an
  array of it) is clang's `RequireLiteralType` error "constexpr variable
  cannot have non-literal type" with "struct/class with virtual base
  class(es) is not a literal type" at the class and "virtual base class
  declared here" per virtual base, in `virtual_bases` order
  (`constexpr_var_virtual_base`); constructing one inside a constant
  evaluation traps in the Oven with clang's `CheckLiteralType` note
  "non-literal type 'X' cannot be used in a constant expression" (WP-146).
  The literal-type check first requires a complete type, so it
  instantiates a class template specialization (WP-170). A `constexpr`
  (or `consteval`) non-static member function, constructor or destructor
  defined in a class with virtual bases is clang
  `CheckConstexprFunctionDefinition`'s "constexpr constructor/member
  function not allowed in struct/class with virtual base class(es)" at
  its definition, with the same notes, and is invalid; it is checked
  outside instantiations only (`check_constexpr_virtual_base`, WP-170).
  A C-variadic base constructor is inlined at each call by codegen
  (codegen.md); a template-variadic one is inherited per specialization,
  as clang.
- **Gaps.** The constexpr-variable check runs with the initializer check,
  so after initializer errors (clang: at the declaration); it covers only
  virtual bases (other non-literal classes keep the "non-constexpr
  constructor" note), and `constinit` keeps the inherited-constructor
  note. For an array aggregate in a constant evaluation kiln names the
  element type, clang the array (`'M[2]'`). In an instantiated class the
  shadows are in declaration order; clang lists those replayed from a
  non-dependent pattern using-declaration in reverse (dump and candidate
  order only). Shadow hiding by the implicit constructors matches clang
  (both declare them at `}` for a class with inherited constructors).

### 4.11 During construction and destruction ([class.cdtor])
Sema warns on a direct call of a pure virtual function from a ctor/dtor of its
class: `call to pure virtual member function 'f' has undefined behavior;
overrides of 'f' in subclasses are not available in the constructor of 'C'`
(+ `'f' declared here`). Dynamic-type rules during construction are the
Oven's and codegen's (WP-076/077, vptr stores in ctors).

### 4.12 Defaulted comparisons ([class.compare]; WP-082)
Modules `kiln_sema/src/class/compare/{mod,default,analyze,synth}.rs`, each
a port of the clang 19 function it names.
- **Categories** (`mod.rs`, [cmp.categories]). `cmp_category_type(kind,
  loc, usage)` is clang's `CheckComparisonCategoryType`: it looks up
  `std::partial_ordering`/`weak_ordering`/`strong_ordering`, requires it
  complete and of the form kiln's evaluation relies on, and records the
  valid result in `Ast.cmp_categories` (`kiln_ast::class::CmpCategories`,
  read by the Oven and codegen). `cmp_category_of(t)` is clang's
  `lookupInfoForType`: a recorded class, or else, by name, a class of
  namespace `std` (through inline namespaces) named like a category, even
  before it is validated. The built-in `<=>` on scalars
  (`builtin_three_way`, [expr.spaceship]/4–8) is in the same module.
  Category types print as `'std::strong_ordering'` (`scoped_class_type`:
  `Elaborated` with the non-inline namespaces as qualifier, types.md R4).
- **Declaration checks** (`default.rs`, clang
  `CheckExplicitlyDefaultedComparison`). At `act_on_defaulted` the
  unqualified operator lookups are captured (`capture_cmp_lookups`); the
  checks run at the class's `}` (primaries, then secondaries), or at an
  out-of-class `= default`. An ill-formed comparison is recorded in
  `special.cmp.invalid` rather than marked `INVALID`, because clang still
  considers it as a candidate. The implicit `operator==` of a defaulted
  `<=>` ([class.compare.default]/4) is declared at `}`
  (`declare_implicit_equality`), `default implicit-inline`.
- **Deletedness, category deduction, `constexpr`** (`analyze.rs`, clang
  `DefaultedComparisonAnalyzer`). Overload resolution for each subobject
  comparison `xi @ xi` (opaque lvalues) runs with the defaulted function
  excluded from the candidate set (`OverloadCandidateSet.exclude`,
  clang's `DefaultedFn`) and with the captured lookups. A deleted
  explicitly defaulted comparison warns
  (`-Wdefaulted-function-deleted`) and a use reports
  `sema_ovl_deleted_comparison` with the analyzer's notes.
- **Implicit definition** (`synth.rs`, clang `DefineDefaultedComparison`
  and `DefaultedComparisonSynthesizer`). A defaulted comparison is defined
  on odr-use (`mark_function_referenced` and `build_operator_call` call
  `define_cmp_on_use`, outside templates and unevaluated operands, and not
  from its own body) before its exception specification is resolved, so
  errors in the body get the note `in defaulted ... comparison operator
  for 'X' first required here` at the use. The Oven bridge defines one it
  meets undefined (`oven_bridge.rs`). The bodies match clang's AST
  (goldens under `tests/conformance/ast/class.{eq,spaceship,
  compare.secondary,compare.default}/`). The implicit exception
  specification comes from the body.
- **Synthesized three-way comparison** (`three_way.rs`, clang
  `BuildSynthesizedThreeWayComparison`; WP-456). In a defaulted `<=>`
  whose declared return type `R` is a comparison category, a subobject
  pair `a`, `b` with no usable `<=>` but usable `==` and `<`
  ([class.spaceship]/1) is compared by `a == b ? R::equal : a < b ?
  R::less : R::greater` (`equivalent` for `equal` unless `R` is
  `strong_ordering`; for `partial_ordering`, `... : b < a ? greater :
  unordered`). `synth_three_way(f, l, r, loc)` builds it as a
  `PseudoObjectExpr` (ast.md §5.3): syntactic form `a <=> b` (a
  `BinaryOperator` of the result's type, never evaluated), semantic
  expressions `[OVE(a), OVE(b), chain]` with result index 2; the chain
  is an lvalue of the category's `const` static members, referenced
  without a location as clang does. The analyzer (`analyze.rs`) keeps
  clang's order: a `==`/`<` that is missing or deleted deletes `f` with
  clang's notes (`three-way comparison cannot be synthesized because
  ...`); a `==`/`<` whose result is not contextually convertible to
  `bool` is the conversion error when `f` is defined on use, with the
  `in defaulted three-way comparison operator for 'X' first required
  here` note. A deduced return type (`auto`) never synthesizes: a
  subobject whose `<=>` is missing deletes `f`, and one whose `<=>` is
  still undeduced is `sema_defaulted_cmp_cannot_deduce_undeduced_auto`
  with `note_defaulted_cmp_cannot_deduce_undeduced_auto` and the callee
  note, emitted in the initial (silent) analysis only, as clang.
- **Redeclarations** (WP-456, clang `MergeFunctionDecl` order). An
  out-of-class definition of the implicit `==`, or of a member that was
  explicitly defaulted in its class (`defaulted_member_definition` in
  `class/special/mod.rs`; from `decl.rs`, `declare.rs` and, for explicit
  specializations of class-template members, `templ/funcs.rs`), is
  `sema_define_implicit_member` (select `function` for a comparison) or
  `sema_define_explicitly_defaulted_member`, before the exception-spec
  and `constexpr` checks. A redeclaration of a defaulted comparison
  (e.g. a friend redeclared after its in-class `= default`) first
  resolves the comparison's deferred exception specification
  (`resolve_cmp_exc`), so a missing `noexcept` is diagnosed as clang
  does; the `constexpr` mismatch ([dcl.constexpr]/1) then applies as to
  any function.
- **Templates.** Defaulted comparisons of class templates (member and
  friend) are instantiated with the class and checked at the
  specialization's `}`. Friend functions of class templates are
  instantiated through their `FriendDecl` only (sema-templates.md §4.8).
  The implicit `==` of a pattern's defaulted `<=>` is not instantiated as
  a member (`templ/class.rs` skips it, and an implicit friend
  `FriendDecl`); `declare_implicit_equality` declares it at the
  specialization's `}` and links it to the pattern's implicit `==`
  (`CompareState.implicit_eq`, pattern `<=>` -> its `==`) with
  `member_of_spec`, so `instantiated_from` and an explicit specialization
  (`template<> bool D<long>::operator==(...)`) find it (WP-456). The `==`
  takes the `<=>`'s trailing requires-clause
  ([class.compare.default]/4), as the pattern's for the constraint check.

## 5. Object construction and temporaries

### 5.1 Node payloads (ast.md §5.1 `ExprNode`)
| kind | `a` | `b` | `aux` / notes |
|---|---|---|---|
| `CXXConstructExpr` | ctor `DeclId` | args `ListId` | kind(2: complete, non-virtual base, virtual base, delegating), `list`, `std_init_list`, `zeroing`, `multiple_candidates`; brace/paren range cold |
| `CXXTemporaryObjectExpr` | as above | as above | written `T(args)`/`T{args}` with ≠1 arg; `ty` = `T` |
| `CXXFunctionalCastExpr` | operand | — | cast kind; `T(x)`, `T{x}` |
| `MaterializeTemporaryExpr` | sub | 0 or `LifetimeExtendedTemporary` `DeclId` | storage duration(2: full-expression, automatic, static, thread), `bound_to_lvalue_ref` |
| `CXXBindTemporaryExpr` | sub | dtor `DeclId` | |
| `ExprWithCleanups` | sub | — | `side_effects` |
| `CXXDefaultInitExpr` | field `DeclId` | used_ctx `DeclContextId` | `rewritten` (always set, as clang 19 dumps it); child = the field's shared NSDMI |
| `CXXDefaultArgExpr` | param `DeclId` | used_ctx `DeclContextId` | owned by sema-overload.md §7: child = the shared `ParamData.default_arg` |
| `CXXThisExpr` | — | — | `implicit` |
| `CXXMemberCallExpr` | callee `MemberExpr` | args `ListId` | object = callee's base |
| `CXXInheritedCtorInitExpr` | ctor `DeclId` | — | kind, `inherited_from_vbase` |
| `CXXParenListInitExpr` | inits `ListId` | array filler / 0 | C++20 paren aggregate init |
| `ArrayInitLoopExpr`, `ArrayInitIndexExpr` | common expr (`OpaqueValueExpr`), element init | — | implicit array copy |
| `CXXTypeidExpr` | operand `ExprId`/`TypeId` | — | `type_operand`, `potentially_evaluated` |
| `CXXDynamicCastExpr` | operand | path `ListId` | cast kind (`Dynamic` or `DerivedToBase`/`NoOp`) |

`LifetimeExtendedTemporary` decl payload: `{ extending: DeclId, mangling_index:
u32, value: Option<ConstValueId> }` (the value is written by the Oven for
constant-initialized static-storage references, ast.md §9).

### 5.2 Construction
`build_construct(ty, ctor, args, kind, flags, loc)` is the only way sema forms
a constructor call. Constructor selection is S3O's `resolve_ctor(ty, args,
CtorInitKind, range)` and, for copy-initialization from another type,
`resolve_conversion` (sema-overload.md §10.2); `perform_init` calls them for
class targets, and the S3C query `Ast::constructors(record)` replaces
`has_constructors_or_unknown`. WP-068a runs after 067b (S3O decision 10), so
until then a unique viable non-template candidate is selected, else
`sema_unsupported` (Q7). The selected ctor is access-checked
(`calling a private constructor of class 'S'`), deletedness-checked, marked
referenced and, if implicit, defined (§4.5). No `elidable` flag (C++17 has
guaranteed elision). Abstract class construction except as a base subobject is
an error (§3.4).

### 5.3 Temporaries and cleanups (clang `MaybeBindToTemporary`, `MaybeCreateExprWithCleanups`)
- `Sema.cleanup: CleanupState { needs: bool, side_effects: bool }` is saved and
  reset on entering each full-expression context (function body statements,
  NSDMI, default argument, each mem-initializer, each variable initializer,
  lambda body) and restored on exit.
- `maybe_bind_temporary(e)`: when `e` is a prvalue of class type whose
  destructor is non-trivial, and not the top-level operand of `decltype`
  ([dcl.type.decltype]/2): check the destructor (access: `temporary of type 'S'
  has private destructor`; deleted), mark it referenced, wrap
  `CXXBindTemporaryExpr(e, dtor)`, set `cleanup.needs = cleanup.side_effects =
  true`. Call sites exactly as clang: results of calls (`CallExpr`,
  `CXXMemberCallExpr`, `CXXOperatorCallExpr`) returning a class prvalue,
  `CXXTemporaryObjectExpr`, functional casts and conversion results that
  construct a temporary, `InitListExpr` temporaries of class type. Not applied
  to a `CXXConstructExpr` that initializes a declared object, member or base.
  Clang binds a call result even when it then initializes a variable by
  guaranteed elision (`S s2 = make();` dumps `ExprWithCleanups >
  CXXBindTemporaryExpr > CallExpr`); kiln does the same and the Oven/codegen
  treat such a bind as a no-op.
- A by-value class parameter is a temporary the caller destroys at the end of
  the full-expression ([expr.call]/7; clang `shouldBindAsTemporary` is true
  for a parameter entity). This holds however the parameter is initialized:
  from an expression (`convert_to_class` / `construct_from_class` with
  `ConvCtx::Argument`), or copy-list-initialized from a braced-init-list
  argument or a braced default argument. In the braced case
  `init_list_entity` binds its prvalue result when the context is
  `ConvCtx::Argument` (WP-310; before that fix the parameter of `f({1, 2})`
  was never destroyed;
  `tests/conformance/expr.call/run-braced-arg-temporary-destroyed.cpp`).
- `materialize_temporary(e, bound_to_lvalue_ref)` (temporary
  materialization, [conv.rval]; WP-067b owns it and the
  `MaterializeTemporaryExpr`/`CXXBindTemporaryExpr`/`ExprWithCleanups`
  nodes, and S3O's `perform_ics` calls it, sema-overload.md decision 4)
  wraps a prvalue in `MaterializeTemporaryExpr` with storage duration full-expression when a
  glvalue is needed: reference binding, member access or member call on a
  prvalue (`S().self()` → xvalue), subscript of an array prvalue, `typeid` of a
  prvalue class, discarded-value expressions where clang materializes.
- At `act_on_full_expr` (M2): if `cleanup.needs`, wrap in `ExprWithCleanups`
  (aux `side_effects`) and reset.
- A constant-expression finished by `act_on_constant_full_expr` while an
  enclosing full-expression is being built (a template argument converted
  mid-expression: `use(D(3)) + C<1>::v`, `f<1>(D(3))`, a defaulted
  `enable_if_t<..., int> = 0` parameter substituted by overload
  resolution) keeps that full-expression's pending cleanups when it has no
  temporaries of its own (`has_own_temporaries`: no
  `MaterializeTemporaryExpr`, `CXXBindTemporaryExpr`, `CXXDefaultArgExpr`
  or `CXXDefaultInitExpr` outside a nested `ExprWithCleanups`), so the
  enclosing temporaries are still destroyed at its end ([intro.execution]/5;
  clang's `ConstantEvaluated` context restores `ParentCleanup`; WP-296,
  `tests/conformance/class.temporary/run-template-argument-keeps-full-expression-cleanups.cpp`).
  One with temporaries of its own gets its own `ExprWithCleanups` and then
  restores the enclosing state from before it began (WP-301,
  `tests/conformance/class.temporary/run-constant-expression-own-temporaries-order.cpp`):
  - entering a constant-evaluated context in the parser
    (`enter_constant_evaluated`) records the cleanup state. Leaving it
    (`exit_constant_evaluated(e)`) files that state under the parsed
    constant-expression `e` in `constant_outer_cleanups`, but only when the
    state has pending cleanups;
  - the substituter does the same for an expression template argument
    (`use(D(N + 4)) + C<D(N).v>::v` in a function template);
  - `act_on_constant_full_expr` finds the entry inside its (converted)
    operand, removes it, and restores that state after the constant
    full-expression has consumed the merged state;
  - a default template argument is substituted and converted between
    `push_cleanups` and `pop_cleanups` (clang `SubstDefaultTemplateArgument`
    enters a `ConstantEvaluated` context), so its temporaries never touch
    the call that uses it (`g(D(6))` with `int N = D(5).v`).

  With no entry (no pending enclosing cleanups) the constant
  full-expression takes and resets the state as any full-expression does.

### 5.4 Lifetime extension ([class.temporary]/6–8)
When a reference is bound to a temporary — the result of a temporary
materialization reached through parentheses, `.`/`[]` subobject access,
`const_cast`/`static_cast`/`dynamic_cast`/`reinterpret_cast` without
user-defined conversion that preserve the object, the right operand of a comma,
either branch of `?:`, or a braced aggregate initializer's element — every such
`MaterializeTemporaryExpr` gets a `LifetimeExtendedTemporary` decl (mangling
index per extending decl, codegen `_ZGR`) and the extending decl's storage
duration. Applies to variables, references in aggregate-initialized objects
(`const T& r = A{ S() }.ref` cases per clang), and the backing array of
`std::initializer_list` (§5.7). Not extended (errors/warnings as clang): return
of a reference to a temporary (`returning reference to local temporary object`:
an error in C++26, P2748R5 [stmt.return]/6; before C++26 the same text as a
warning [-Wreturn-stack-address], clang's `warn_ret_local_temp_addr_ref`,
WP-491), a mem-initializer reference member (§4.9), a
temporary in a `new`-initializer or a function argument (full-expression
lifetime). A `.` member access is followed only when it designates a
non-static data member of non-reference type ([class.temporary]/6.7): a
reference member denotes what it was bound to, so `int &r = make().first;`
(with `int &first`) extends nothing and `return make().first;` is not
`returning reference to local temporary object`, for the extension, the
return check and the `-Wreturn-stack-address` walk alike (WP-276).

**P2718R0 (C++23 range-for)**: temporaries in the *for-range-initializer* are
extended to `__range` (clang 19 dump: `MaterializeTemporaryExpr 'S' xvalue
extended by Var '__range1'`). Sema collects every full-expression-duration
`MaterializeTemporaryExpr` created while parsing the range-initializer
(`Sema.extend_collector: Option<Vec<ExprId>>`, clang
`InLifetimeExtendingContext`), including those in default arguments of calls in
it, and extends them all to `__range`.

Status (WP-089): implemented. `Sema::begin_for_range_init(extend)` opens a
collection (`extend` is `Sema.range_lifetime_extension`, set by the parser for
`-std=c++23` and later) and `end_for_range_init` closes it, returning the
collected temporaries; `materialize_temporary` pushes each one it creates onto
the innermost open `extend_collector`. Temporaries already bound to a reference
inside the initializer are left alone. After `__range` is initialized each
collected temporary still of full-expression duration is extended to
`__range` (`extend_temporary`, so it gets the `extended by Var '__range1'`
dump, a mangling index, and the variable's storage duration). Codegen and the
Oven destroy the extended temporaries at the end of the loop, after `__range`'s
own temporary, in reverse order of construction. Before C++23 nothing is
collected, and only the temporary bound directly to `__range` is extended.
Templates collect again when the range is substituted (`templ/subst/stmts.rs`
`range_for`).

**Returned locals (WP-204, `class/temporary/stack_addr.rs`).**
`Sema::check_return_lifetime(c, ret)` runs on every converted
non-dependent return operand (clang `checkExprLifetime`, `LK_Return`). A
reference return first gets the direct-temporary check above
(`check_dangling_temporary`); then the operand is walked like clang's
`visitLocalsRetainedByReferenceBinding` (reference return: through
parens, no-op / derived-to-base casts, `.` member access, subscripts of
arrays, `*` of a pointer, the comma's right operand, both `?:` arms, and
the initializer of a local reference) or `visitLocalsRetainedByInitializer`
(otherwise: `&`, array decay, no-op / bit / base casts, pointer `+`/`-`,
both `?:` arms, the initializer of a `const` local read by
lvalue-to-rvalue conversion, and each capture initializer of a lambda).
Each automatic variable or parameter reached is `{address of|reference to}
stack memory associated with {local variable|parameter} 'x' returned`
[-Wreturn-stack-address] at the first reference/`const` variable or
capture on the path (else at the name), with one note per path step:
`binding reference variable 'r' here` / `via initialization of variable
'p' here`, `captured by reference here`, `implicitly captured by
reference due to use here` (located at the capture-default), `captured by
reference via initialization of lambda capture 'r'`. A temporary reached
through a local reference is `returning reference to local temporary
object` (reference return; an error in C++26, a warning before, as
above) or `returning address of local temporary
object`, with the same notes. A GNU `&&label` reached the same way is
`returning address of label, which is local` [-Wreturn-stack-address]
(WP-424). Reference parameters, statics,
`->` access, captured variables named inside a lambda and aggregate
(non-lambda) prvalues are not followed. The warning is
`per_instantiation` (diag.md §7.4): as in clang, a non-dependent return
warns in the template definition and once more in each instantiation
(WP-231).

**Dangling local pointers (WP-227, clang `LK_Extended` on a pointer
variable).** `check_dangling_pointer_var(v, init)` (`stack_addr.rs`) runs
after lifetime extension on the initializer of a non-dependent
pointer-typed variable (through `ExprWithCleanups` and a one-element
braced list): it is walked like a returned value, and each temporary
reached only through `&`/array decay (`const int* p = U{1, 2};`, a
member array of a class prvalue, a `?:` arm) is `temporary whose address
is used as value of local variable 'p' will be destroyed at the end of
the full-expression` [-Wdangling] at the temporary. A lifetime-extended
temporary or a named array is not. Not covered: a reference variable
bound through a pointer (`const int* const& r = U{5};`, clang's `bound
to local reference`), assignments (`-Wdangling-assignment`) and pointer
members with an NSDMI.

### 5.5 Copy elision and NRVO ([class.copy.elision])
- **Guaranteed elision** ([dcl.init.general]/16.6.1): a prvalue of the same
  class (ignoring cv) initializes the object directly; no copy or move
  `CXXConstructExpr` is formed.
- **No elision into a base or delegating target** (CWG2403; clang
  `RequireActualConstructor`, WP-158): a prvalue initializing a base
  subobject (a mem-initializer, or an aggregate's base element, whose
  construction kind is `NonVirtualBase`) or the target of a delegating
  constructor is materialized and copied or moved by a real constructor;
  CWG2327 conversion-function candidates are not added for such targets
  (`resolve_ctor_ex(.., actual_ctor: true)`).
- **Implicit move** (C++23 [expr.prim.id.unqual]/4): in `return` and
  `co_return` operands an id-expression naming an implicitly movable entity is
  an xvalue (clang 19 dump: `ImplicitCastExpr 'const S' xvalue <NoOp>` before
  binding to `const S&`). The entity is a parameter or a block-scope
  variable with automatic storage duration ([class.copy.elision]/3): a
  block-scope `thread_local` (with or without `static`) has thread
  storage duration and stays an lvalue, so `thread_local int i; return
  i;` binds an `int&` return (`has_automatic_storage`; WP-533,
  `basic.stc.thread/local-lvalue-run.cpp`).
- **NRVO marking (clang 19 `Scope::updateNRVOCandidate` / `applyNRVO` /
  `computeNRVO`).** A copy-elision candidate of `return e` is `e` (through
  parentheses) naming a non-volatile, non-reference local variable with
  automatic storage (not a parameter, not `thread_local`, not an exception
  variable) whose type is the class return type ignoring cv (or either is
  dependent). `ReturnStmt.b` = that candidate (dump `nrvo_candidate(Var 'x'
  'S')`). Each scope carries `nrvo: NrvoState { slots: Vec<DeclId>, nrvo:
  Option<Option<DeclId>> }` (clang `ReturnSlots` / `NRVO`): every local
  variable is added to its scope's `slots` when declared (`push_local`).
  At a return with candidate `c` (possibly none), walking from the current
  scope out to and including the function scope, each scope keeps only `c`
  in its `slots` if `c` was there and drops the rest; the innermost scope's
  `nrvo` becomes `Some(Some(c))` if `c` is in its slots (or declared
  outside it) and the scope had no other verdict, else `Some(None)`
  (disabled). On scope exit (`pop_scope` → `apply_nrvo`): `Some(Some(x))`
  with `x` declared in this scope sets `x`'s `nrvo` bit; a block/control
  scope's verdict otherwise overwrites its parent's. The function scope's
  own verdict is applied first in `end_function_body`; then, for a
  non-template function with a class return type, every return in
  `FnState.returns` whose candidate did not end up `nrvo` has its candidate
  cleared (`AstBuilder::clear_return_nrvo`, clang `computeNRVO`). Probes (all
  clang 19, golden `ast/class.copy.elision/nrvo.cpp`): `if (b) { S x; return
  x; } S y; return y;` → both `nrvo`; `S x, y; if (b) return x; return y;` →
  neither; `S x; if (b) return S(); return x;` → neither; `return p;`
  (param) → not a candidate. Alignment (`alignas` on the variable) is not
  yet considered (clang `isCopyElisionCandidate` checks it).

### 5.6 Class initialization forms (extends `init.rs`, sema.md §6)
| form | class `T` |
|---|---|
| default-init | default ctor `CXXConstructExpr`, also when trivial (as clang); a const object needs `CAN_CONST_DEFAULT_INIT` (`default initialization of an object of const type 'const T' without a user-provided default constructor`) |
| value-init `T()` / `{}` | no user-provided default ctor: zero-initialize then construct (`zeroing`); user-provided: call it; aggregate `{}`: `InitListExpr` |
| direct-init `T(args)` | [over.match.ctor]; same-class prvalue elides; C++20 paren aggregate init → `CXXParenListInitExpr` when no ctor is viable and `T` is an aggregate |
| paren aggregate init, elements (WP-182) | each element copy-initialized from its argument; a braced-init-list argument (`int a[2][2]({1, 2}, {3})`, also in a new-initializer) copy-list-initializes its element. The filler of an array (or a value-initialized member) whose element is an array of classes is ONE `CXXConstructExpr` of that array type (`zeroing` unless the default ctor is user-provided), as clang; codegen runs it as the array-constructor loop. A lone braced list in parentheses for a scalar or reference is clang's `cannot initialize {non-class,reference} type T with a parenthesized initializer list` at the entity. Order (WP-316, clang `TryOrBuildParenListInitialization`): the elements are initialized in order and the first failure ends it (its error at the argument; an array argument that is not a braced list is `array initializer must be an initializer list`, a `RecoveryExpr`); only then are excess arguments `excess elements in {array,union,struct} initializer` at the entity. An array initializes every argument as an element, past the bound too; a union its first member from the first argument; a union without members takes none. A single-operand cast `T(x)`, `(T)x`, `static_cast<T>(x)` to a class (aggregate or not) with no viable constructor is `no matching conversion for {functional-style cast,C-style cast,static_cast} from 'F' to 'T'` (`ambiguous conversion for ...` when ambiguous) at the cast, with the constructor candidate notes (clang `tryDiagnoseOverloadedCast`, `DeletedAs::Cast`); for an aggregate the paren aggregate init is tried first in verify-only mode (`verify_paren_aggregate_init`, sema.md §6 "Verify-only initialization": no instantiation, no implicit member definition, no diagnostic survives the attempt) and, if it fails, the cast is that constructor diagnostic (`FK_ParenthesizedListInitFailed` reports the constructor overload failure); if it succeeds the init is built for real, so its warnings and instantiations happen once. Value-initialized array-of-class members of `B(1)` (`struct B { int i; S arr[2]; }`) are fine in a template too; clang 19 rejects the instantiated `return B(1);` (fixed by clang 22; deliberately not copied, docs/VALIDATION.md). Tests: `dcl.init.general/paren-aggregate-{element-errors-first,array-of-class,cast-verify-only,cast-run}.cpp`, `expr.static.cast/cast-no-matching-conversion.cpp` |
| copy-init from `T`/derived | elision for prvalue `T`; else copy/move ctor (derived glvalue: `DerivedToBase` then ctor) |
| copy-init from other type | user-defined conversion (S3O): `ConstructorConversion` around `CXXConstructExpr`, or `UserDefinedConversion` around the conversion call; `explicit` ctors excluded (`no viable conversion from 'int' to 'T4'` + candidate notes incl. `explicit constructor is not a candidate`) |
| list-init, aggregate | `InitListExpr`: C++17 base elements first (each initialized from its element, brace elision applies), then fields; designated initializers name direct members only, bases are then initialized from `{}`. A base element's outermost constructor call is `NonVirtualBase` (clang `EK_Base`; codegen calls C2) when it copies/moves a source of the base or a derived class (a same-class prvalue is materialized first, §5.5) or braced/value-initializes (or fills) a non-aggregate base (`as_base_subobject`); a converting constructor's temporary (`D{5, 6}`) stays `Complete`. Paren aggregate init does the same |
| list-init, non-aggregate | at `-fclang-compat<22` (clang 19), a single undesignated element of `T` or a derived class first (DR1467 as clang 19 applies it to every class): initialized from it as from `T x = e` (copy-list) or `T x(e)` (direct-list), so no initializer-list ctor is considered and a copy-list init never picks an explicit ctor; the elided prvalue carries no `InitListExpr` wrapper and the ctor no `list` flag (clang's `RewrapReferenceInitList` / `IsInitListCopy`, a dump-only deviation shared with the aggregate case). Otherwise [over.match.list] two phases (initializer-list ctors, then all ctors); `CXXConstructExpr` with `list`; narrowing as M2; a nested braced-init-list element (`G g{{1, 2}}`) is a braced call argument converted by [over.ics.list] (WP-152); designators remain `sema_unsupported`. At `-fclang-compat>=22` (clang 20+, CWG2137 [over.match.list], [over.ics.list]) the shortcut is gone: [over.match.list] runs first, so `B{b}` picks `B(std::initializer_list<B>)`; only when no initializer-list ctor is viable, a single element that is a prvalue of `T` (cv-unqualified same class) initializes the object directly (elision, [dcl.init.general]/17.6.1), outside base-subobject init. Tests: `dcl.init.list/dr1467-single-element{,-clang19}.cpp` |
| reference to class | derived-to-base glvalue binding, else temporary through conversion (`materialize_temporary`, §5.3) |

### 5.7 `std::initializer_list`
`CXXStdInitializerListExpr` over a materialized backing array
(lifetime-extended like a temporary, [dcl.init.list]/6) needs the
`std::initializer_list` class template (S3T instantiation) and phase 1 of
[over.match.list]; it lands with WP-068a after WP-070. Before that,
list-initialization that would consider an initializer-list ctor is
`sema_unsupported("std::initializer_list")`.

### 5.8 `dynamic_cast` and `typeid` (sema WP-078a; parsing WP-064)
`dynamic_cast` ([expr.dynamic.cast]): upcasts are `DerivedToBase`/`NoOp`;
`void*` targets, downcasts and cross-casts need a polymorphic complete source
(`'A' is not polymorphic`) → `Dynamic`. `typeid` ([expr.typeid]) needs
`std::type_info` declared (`you need to include <typeinfo> before using the
'typeid' operator`); only a glvalue of polymorphic class type is potentially
evaluated; the result is an lvalue `const std::type_info`.

## 6. Record layout (`kiln_abi`, abi.md §8)

### 6.1 Inputs
All from the `Ast` (kiln_abi never sees sema): `RecordData.bases`, definition
bits (`DYNAMIC`, `POLYMORPHIC`, `HAS_VIRTUAL_BASES`, `EMPTY`), fields and their
flags, `alignas`/`packed` attributes, and the `#pragma pack` value current at
the class definition, which sema records in cold map `record_pack: DeclId →
u16` (abi.md §8.2 sources `ClassDesc.max_field_align` from it).

**`#pragma pack` state (WP-066c; clang `Sema::ActOnPragmaPack`).**
- **State.** Sema keeps the current value N (bytes, 0 = none) and a
  stack of `(label, value)` entries.
- **Applying it.** `act_on_start_record_definition` stores a nonzero N
  in `record_pack` for the class being defined. A class template's
  pattern gets it too; instantiation must copy it from the pattern
  (WP-066b/templates; not done yet).
- **Forms** (the argument text of `#pragma pack(...)` and
  `_Pragma("pack(...)")`):
  - `()` resets N to 0;
  - `(N)` sets N;
  - `(push)`, `(push, id)`, `(push, N)` and `(push, id, N)` push the
    current value (with `id`), then set N if given;
  - `(pop)` restores the top entry;
  - `(pop, id)` pops through the nearest entry labelled `id`, or does
    nothing if there is none;
  - `(pop, N)` pops, then sets N.
  - N must be one of 0, 1, 2, 4, 8 or 16.
- **Rejected forms.** These are reported through the
  unsupported-pragma diagnostic (`#pragma pack ... is not supported
  yet`; an error), where clang 19 warns and ignores them:
  - `show`;
  - any other N;
  - `pop` on an empty stack;
  - `pop` with both a label and N;
  - an unknown action.
- **Not implemented.** clang's end-of-file `-Wpragma-pack` "unterminated
  push" warning and the include-boundary warnings are missing.

### 6.2 Algorithm (Itanium §2.4, clang `ItaniumRecordLayoutBuilder`)
Status: steps 3–5 and 7 are implemented for non-virtual bases (WP-066c,
abi.md §8.1). Steps 1, 2 and 6 and the vptr are WP-078b (abi.md §8.5).

1. **Primary base** (II.1): the first non-virtual dynamic base; else the first
   nearly-empty virtual base that is not an indirect primary base; else the
   first nearly-empty virtual base that is (an indirect primary).
2. **vptr** at 0 (8 bytes, align 8) if dynamic without a primary base.
3. **Non-virtual bases** (II.2): the primary at 0, then the rest in declaration
   order; an empty base at offset 0 unless the empty subobject map conflicts
   (then at `dsize` aligned, retrying upwards); a non-empty base at
   `align_to(dsize, nvalign)`, retrying while it conflicts. A base adds its
   `nvsize` to `dsize` (and `size`), its `nvalign` to `align`.
4. **Fields** as M2, starting at `dsize` (tail padding of a non-POD base is
   reused; bit-fields may begin in it). Potentially-overlapping fields
   (`[[no_unique_address]]`) of class type are placed like bases (nvsize/dsize,
   empty at a non-conflicting offset; probe: `struct R : E {
   [[no_unique_address]] E e; int r; }` → base E at 0, `e` at 1, `r` at 0,
   sizeof 4).
5. `nvsize = dsize`, `nvalign = align`.
6. **Virtual bases** (III): in inheritance-graph order, each not already placed
   as a primary base, by the step-3 rule at offsets ≥ `dsize`.
7. `size = align_to(max(size, dsize), align)`. A class failing the
   POD-for-layout test (abi.md §4) exposes its tail padding (probe: `P { int
   i; char c; P(); }` dsize 5; `Q : P { char q; }` puts `q` at 5, sizeof 8).

`RecordLayout` (types.md §11) fills `bases`, `vbases`, `vptr`, `primary_base`;
`EmptySubobjectMap` gains base subobjects (`CanPlaceBaseAtOffset`) and the
largest-empty-subobject cut-off.

### 6.3 Dump (`-fdump-record-layouts`, byte-identical)
As observed: base lines `{off} |   struct B (primary base)` / `(base)` /
`(virtual base)`, the vptr as `(B vtable pointer)` at the depth of its class,
two spaces of indent per nesting level, `(empty)` after an empty class's name,
virtual bases of a base printed once, at the end of the most-derived layout
(`struct D` prints `struct A (virtual base)` after `int d`, not inside
`B`/`C`), footer `[sizeof=48, dsize=44, align=8,` / `nvsize=32, nvalign=8]`.
Clang's plain flag dumps each record when first laid out (bases before
derived). The driver's dump instead follows clang's
`-fdump-record-layouts-complete` order: every complete class, in lexical
post-order, each named as at its `}`. The corpus differential compares
against `-complete` block by block, in order (abi.md §8.4).

### 6.4 Differential tests
- `kiln_abi/tests/layout_vs_clang.rs` (abi.md §7) extends its generator
  (`KILN_ABI_DIFF_SEED/RECORDS/SHOW`): hierarchies of depth ≤ 4 with ≤ 4 bases
  each, `virtual` with p = 0.3 (after WP-078b; 0 before), empty classes,
  repeated bases, polymorphic classes, non-POD bases with tail padding,
  `[[no_unique_address]]`, bit-fields after bases, `alignas`, `#pragma pack`.
  CI: fixed seed, 2000 records; XFAIL list empty. WP-066c implements this
  for non-virtual bases and `#pragma pack` (1–3 bases per class, depth
  bounded by the item count). Virtual bases and polymorphic classes come
  with WP-078b.
- `kiln_abi/tests/vtable_vs_clang.rs` (WP-078b): `-fdump-vtable-layouts` vs
  `kiln_abi::dump_vtable_layout`, byte-identical (`Vtable for 'B' (13
  entries).`, `vbase_offset (8)`, `-- (A, 8) vtable address --`, `[this
  adjustment: 0 non-virtual, -24 vcall offset offset]`, and the `Thunks
  for`, `VTable indices for`, `Virtual base offset offsets for` tables); same
  generator, 500 classes.
  Landed (WP-078b):
  - `layout_vs_clang` runs 2000 records per target with virtual bases
    (p = 0.3) and polymorphic classes (p = 0.2). Its XFAIL list is
    empty.
  - `vtable_vs_clang` runs 500 classes per target through its own
    polymorphic generator. It compares every vtable and construction
    vtable dump, and every VTT against clang's `@_ZTT` IR. Its XFAIL
    list is empty.
  - `vtable_rules.rs` adds the hand-written thunk, VTT and destructor
    probes.
  - See abi.md §7.
- `kiln_abi/tests/layout_rules.rs`: one named test per probe in §6.2.

### 6.5 Layout caches (WP-450, WP-428)
A class's layout is computed once per TU wherever it is needed more than
once; a cache never holds the layout of an incomplete class, which may
still change ([basic.types.general]/5), and is never invalidated.
- **`Ast::layouts`** (spec/types.md §11): `kiln_abi::ensure_layout`, for
  the Oven (`sizeof`, `alignof`, `offsetof`, member offsets) and codegen.
  It lays out complete classes only (`None` for an incomplete one). Sema's
  `sizeof`/`alignof` only check the operand; the Oven folds the value.
- **`Sema::layout_cache`** (a `kiln_abi::LayoutCache`): the alignment of a
  class with a virtual destructor, which selects its deallocation function
  ([expr.delete]/10, [class.dtor]/16; `complete_class_align`, called by
  `check_destructor_delete` and `virtual_dtor_delete_usable`). An implicit
  virtual destructor is declared, and its deletion decided, while the class
  is still being defined, so that class is laid out over the cached layouts
  of its complete bases and members and then dropped
  (`Layouter::into_complete_cache`, abi.md §1): each class of a hierarchy is
  laid out once, not once per derived class. Before WP-428 a class still
  being defined was laid out from scratch with all its bases, cubic in the
  length of a chain of polymorphic classes.
- Other sema queries (`crate::target::size_align`: bit-field widths,
  `alignas` checks, atomics, `__builtin_*` operand checks) lay out from
  scratch: each is asked about a few types per declaration, not once per
  class of a hierarchy.

Validation: `bench/frontend/virtual-dtor-chain.cpp` (600 classes: 3.8 s
before WP-428, 0.28 s after, clang 22 about 1.1 s; its `threshold-time`
baseline, `threshold-time: 100`, catches a return of the per-class layout) and the virtual lattice
below (§10).

## 7. Diagnostics
- **Wording** is clang 19's, verified on the container; every quoted message in
  §2–§5 is normative (text, argument formatting, notes and their order). The
  ids go into `kiln_diag::ids` as `sema_<area>_<what>` with areas `member`,
  `access`, `base`, `virtual`, `special`, `init`, `temp` (e.g.
  `sema_access_private`, `sema_special_implicitly_deleted_call`); one id per
  clang diagnostic, `{n}` arguments in clang's order, `{n:a|b}` selects where
  clang uses `%select` (constructor/destructor/assignment names, `private`/
  `protected`, `class`/`struct`).
- **Kinds**: everything quoted is an `Error` except the warnings named with
  their group: `-Wreorder-ctor`, `-Wuninitialized`, `-Woverloaded-virtual`,
  `-Winconsistent-missing-override`, `-Wdefaulted-function-deleted`,
  `-Wcall-to-pure-virtual-from-ctor-dtor`, `-Wdelegating-ctor-cycles`
  (`WarningError`), `-Wclass-conversion`, `-Wreturn-stack-address`;
  `-Wsuggest-override` is `WarningOff`.
- **Notes** point at the named declaration; implicit-definition context
  notes (`in implicit copy constructor for 'T' first required here`) follow
  diag.md's context-note order.
- **Recovery**: an invalid base is dropped; an invalid member is `INVALID`
  but stays in lookup; a class with one is complete, non-trivial and
  non-aggregate; an ill-formed implicit definition marks the member
  `INVALID` and later uses are silent; access errors never change the
  selected entity.

## 8. Interfaces

### 8.1 `kiln_ast` additions (WP-066a)
- `DefinitionBits` 24–49 (§2.9); `FnSpec` bits 18 `override`, 19 `final`, 20
  `explicit_object`, and `deleted` joins `FnSpec::LATE` (`= default` members
  are found deleted at `}`) — **public-interface change**.
- `ParamData` flag `DEFAULT_ARG_PENDING`; `LifetimeExtendedTemporary`
  payload (§5.1). Hidden friends use `LookupFlags::ADL_ONLY`, owned by
  sema-overload.md (§2.3); S3C adds no lookup bit.
- Storage (`HeapSize` rows): arena `ctor_inits` + cold map; `overrides`,
  `friends` (`ListId` per key); cold maps `key_function`, `dtor_delete`,
  `record_pack`, `explicit_spec`, `deleted_msg`; insert-only `virtuals` and
  `vtables` (`kiln_abi` type, stored like `layouts`).
- `AstBuilder`: `access_spec`, `friend`, `set_ctor_inits`, `set_overrides`,
  `lifetime_extended_temporary`, `append_implicit_member`, `set_nrvo`, and one
  constructor per §5.1 expression kind.
- Reads: `Ast::destructor(record)`, `Ast::constructors(record)` (table reads,
  never declare), `Ast::ctor_inits(ctor)`.
- Dump lines and normalizer rows (`xtask/src/clang_ast/table.rs`) for
  `CXXMethodDecl`, `CXXConstructorDecl`, `CXXDestructorDecl`,
  `CXXConversionDecl` (`constexpr`, `explicitlyDefaulted`, `explicitlyDeleted`,
  `inline`, `virtual`, `pure`), `AccessSpecDecl`, `FriendDecl`,
  `CXXCtorInitializer`, record `bases` (`access`, `writtenAccess`,
  `isVirtual`), `FieldDecl` `hasInClassInitializer` and `mutable`, and §5.1
  keys (`ctorType`, `list`, `constructionKind`, `hadMultipleCandidates`,
  `extendingDecl`, `storageDuration`, `boundToLValueRef`, `temp`, `dtor`,
  `cleanupsHaveSideEffects`, `hasRewrittenInit`, `implicit`).

### 8.2 Mutation table additions (ast.md §2.4)
| slot | written | by |
|---|---|---|
| `RecordData.members.last` of a complete class | append-only, implicit special members and inheriting ctors | sema |
| `VarData` `nrvo` bit | once, at the variable's scope exit | sema |
| ctor-init list of a ctor | once, with the body | sema |
| `LifetimeExtendedTemporary.value` | once | Oven via bridge |
| `FnSpec::deleted` | once, at `}` (`= default` members) | sema |

### 8.3 `Sema` state additions
| field | purpose |
|---|---|
| `classes: Vec<ClassBuild>` | per class being defined: record, current access, running definition bits, first virtual, NSDMI/default-arg pending counts |
| `implicit_declared: FxHashMap<DeclId, u8>` | which implicit members exist (§4.1) |
| `delayed_access: Vec<AccessCheck>` + depth stack | §2.4 |
| `cleanup: CleanupState` | §5.3 |
| `extend_collector: Option<Vec<ExprId>>` | §5.4 P2718 |
| `unparsed_exc: FxHashSet<DeclId>` | §2.7 |
| `uninstantiated_exc: FxHashMap<DeclId, DeclId>` (WP-112) | members, friends and function template specializations whose dependent `noexcept(expr)` is instantiated when needed: first decl → pattern (sema.md §4.7) |
| `delayed_exc_equiv: Vec<(DeclId, DeclId)>` (WP-269) | redeclarations whose spec was unknown when declared, paired with the previous declaration (sema.md §4.7) |
| `unparsed_default_args: FxHashMap<DeclId, Loc>` (WP-269) | §2.7 |
| `special: SpecialState` (WP-067a) | pending `= default` checks, pending deleted uses, lazily found deleted implicit members, the definition bits of classes at `}`, fields with an NSDMI (§4) |
| `FnState.this_ty`, `this_override` | §2.5 |
| `Scope.nrvo` | §5.5 |
New parser entry points (WP-065's contract): `act_on_base_specifiers`,
`act_on_access_spec`, `act_on_friend`, `act_on_defaulted`, `act_on_deleted`,
`act_on_param_default_arg`, `act_on_exception_spec`, `act_on_field_init`,
`act_on_mem_initializers`, `act_on_finish_class_contexts`, `act_on_this`,
and (added by WP-065) `act_on_class_final` (§2.1), `act_on_start_delayed_class`,
`act_on_finish_delayed_class`, `act_on_start_delayed_method`,
`act_on_finish_delayed_method`, `act_on_start_field_init` (§2.7):
```
fn act_on_class_final(&mut self, r: DeclId, loc: Loc);
fn act_on_start_delayed_class(&mut self, r: DeclId);
fn act_on_finish_delayed_class(&mut self, r: DeclId);
fn act_on_start_delayed_method(&mut self, f: DeclId);
fn act_on_finish_delayed_method(&mut self, f: DeclId);
fn act_on_start_field_init(&mut self, field: DeclId);
```
Modules: `kiln_sema/src/class/{mod,access,special,ctorinit,temporary,virtual}.rs`
(§9).

**Landed signatures (WP-066a, `kiln_sema::class`; bodies report
`{what} is not supported yet` until the owning WP):**
```
pub struct BaseSpecInfo { ty: QualType, loc: Loc, is_virtual: bool, access: Access, pack_expansion: bool, attrs: Vec<AttrId> }
pub enum MemInitId { Ident(Symbol), Type(QualType) }
pub struct MemInitInfo { id: MemInitId, loc: Loc, init: Initializer, pack_expansion: bool }
fn act_on_base_specifiers(&mut self, r: DeclId, specs: &[BaseSpecInfo]);
fn act_on_access_spec(&mut self, access: Access, loc: Loc) -> Option<DeclId>;
fn act_on_friend(&mut self, ds: &DeclSpec, base: QualType, d: Option<&Declarator>, friend_loc: Loc) -> Option<DeclId>;
fn act_on_defaulted(&mut self, f: DeclId, loc: Loc);
fn act_on_deleted(&mut self, f: DeclId, loc: Loc, msg: Option<ExprId>);
fn act_on_param_default_arg(&mut self, p: DeclId, e: ExprId);
fn act_on_param_default_list(&mut self, p: DeclId, l: &InitList);
fn act_on_exception_spec(&mut self, f: DeclId, spec: ExcSpec);
fn act_on_field_init(&mut self, field: DeclId, init: Initializer);
fn act_on_mem_initializers(&mut self, ctor: DeclId, inits: &[MemInitInfo], colon_loc: Loc);
fn act_on_finish_class_contexts(&mut self, r: DeclId);
fn act_on_this(&mut self, loc: Loc) -> ExprId;
fn current_class(&self) -> Option<&ClassBuild>;
```
Parser-facing additions outside `class/`: `ParamInfo.default_arg:
DefaultArg { None | Parsed(ExprId) | Unparsed(Loc /* = */) }` (set by the
parser after `act_on_param_declarator`; a `Parsed` default is already
converted by `act_on_param_default_init` (sema.md §4.3), which
`act_on_declarator` stores, and `Unparsed` parameters are marked
`DEFAULT_ARG_PENDING`), and `TagUse::Friend` for the
elaborated-type-specifier of `friend class X;`. Calling protocol: the
parser calls `act_on_base_specifiers` for every class definition (empty
slice when there is no base-clause), `act_on_mem_initializers` for every
constructor definition (empty slice, `colon_loc` = the body's `{`, when
there is no ctor-initializer) and `act_on_finish_class_contexts` for every
outermost class; those calls are silent on the stubs, so M2 programs are
unaffected. `ClassBuild` is pushed and popped by
`act_on_{start,finish}_record_definition` already.

**Landed (WP-066b).** Modules `kiln_sema/src/class/{members,access,
member_expr,friend}.rs`; the WP-066a stubs for base specifiers, access
specifiers, friends, default arguments, NSDMIs and `this` now have bodies.
- §2.3: static data members (`act_on_static_member`: in-class declaration
  vs `inline`/`constexpr` definition, `static const` integral/enum
  initializers usable in constant expressions, local-class and
  unnamed-class errors, out-of-line `int S::s = 1;` redeclaration whose
  initializer is checked in the class's access context); member function
  declarations (cv/ref-qualifiers, `static` with qualifiers diagnosed,
  in-class definition implicitly inline — `FnSpec::implicit_inline` joins
  `FnSpec::LATE`); out-of-line definitions enter the class scope and its
  bases for the body (`enter_definition_scopes`), and a local class's
  member function body saves the enclosing function (`Sema.fn_saved`).
- §2.4: `check_member_access` / `check_base_access` port `SemaAccess`
  (naming class, [class.access.base]/5 paths, [class.protected], friends
  from `ClassBuild.friends` stored at `}`). `err_access`'s class argument is
  the declaring class (clang). Delay points for the parser:
  `pub fn begin_delayed_access(&mut self)` /
  `pub fn end_delayed_access(&mut self, decl: Option<DeclId>)`.
- §2.5: `act_on_this`; NSDMI replay brackets the initializer with
  `pub fn act_on_start_field_init(&mut self, field: DeclId)` (pushes `C*` on
  `Sema.nsdmi_this`) and `act_on_field_init` (pops it). Implicit `this->`,
  `const` member assignment notes, and "must be called" for bound member
  functions.
- §2.6: `&C::m`, `.*`/`->*` (`act_on_ptr_mem`), member-pointer
  conversions. A name of a non-static member is held in `Sema.member_refs`
  until its operand is known (`&C::m` vs an object use); unresolved
  entries are flushed (diagnosed) at the end of the full-expression.
- §3.1: `attach_bases` (every §3.1 check except virtual bases, which stay
  `sema_unsupported`); §3.2: `Sema::lookup_in_class(c, name, kind) ->
  Found`, with `Found.naming: Option<DeclId>` and `Found.ambiguity:
  Option<MemberAmbiguity { Types(Vec<DeclId>) | Subobjects { decl, paths }
  }>` in place of a separate `MemberLookup` type, diagnosed by
  `diagnose_member_ambiguity`; §3.3: base access and ambiguity checks on
  every derived-to-base conversion.
- Member calls (single candidate until WP-068a): `obj.f(args)` /
  `p->f(args)` is `CXXMemberCallExpr` whose callee is `MemberExpr` of type
  `<bound member function type>` over the object converted to `cv C`
  (`NoOp`, or `UncheckedDerivedToBase` when `f` is in a base; a prvalue
  object is materialized), arguments converted to the parameter types,
  default arguments as `CXXDefaultArgExpr`; a static member function call
  is `CallExpr(FunctionToPointerDecay(DeclRefExpr | MemberExpr))`; a call
  through a member pointer is `CXXMemberCallExpr(ParenExpr(BinaryOperator
  .*|->*))`. More than one candidate is `overloaded member functions is not
  supported yet`.
- `kiln_ast` additions: `DeclArena::set_access(d, Access)`;
  `FnSpec::storage_inherited` (bit 22: `static` inherited from a previous
  declaration, which clang's dump omits; also fixes the free-function
  `static void f(); void f() {}` dump); `ValueCategory` of a non-static
  member function name is prvalue.
- Sema state: `fn_saved`, `member_refs`, `friend_tag`, `nsdmi_this`,
  `initializing` (the out-of-line static member whose initializer is being
  analysed: the [class.access]/7 context).
- Not done (owners): aggregate initialization of a class with bases
  (including `D d{}`) is `sema_unsupported` (WP-067b); the `POD` bit is not
  computed (§2.9); P0847 explicit object parameters wait for WP-065's
  syntax (Q4); virtual bases (WP-078a).

**Landed (WP-078a).** Module `kiln_sema/src/class/virtual/{mod,graph,
complete,abstract_,cast}.rs`; `virtual`, `override`, `final`, pure
specifiers and virtual base-specifiers are no longer `sema_unsupported`.
- §3.4 declarations (`mod.rs`): `virtual_decl(ds, d, ty, VirtualSite,
  &mut FnSpec) -> VirtualDecl` runs before the function is built and
  `finish_virtual_decl(f, VirtualDecl)` after (decl.rs `act_on_function`):
  `virtual` outside a class / on an out-of-line definition / on a friend
  / with `static`, pure-specifier on a non-virtual, `override`/`final` on
  a non-member, the overridden-method search over all bases
  (`overridden_methods`, same name + `is_method_overload` false: same
  parameter-type-list, cv- and ref-qualifiers; operator functions such
  as `operator()` included, WP-276), implicit virtualness,
  `override` that overrides nothing, overriding a `final` function,
  covariant returns ([class.virtual]/8, `check_override_return`, clang
  `CheckOverridingFunctionReturnType`, with [class.access.base]
  `check_covariant_base_access` in `access.rs`) and the overridden-method
  list (`builder().set_overrides`, `cold.overrides`; a redeclaration
  inherits it). `virtual_on_non_function` covers
  `virtual` on typedefs and variables.
- At `}` (`complete.rs`, from `act_on_finish_record_definition`):
  `virtual_class_bits` sets `POLYMORPHIC`, `HAS_VIRTUAL_BASES` and
  `ABSTRACT` before completion; `act_on_complete_virtual` then reports
  multiple final overriders (`sema_multiple_final_overriders` +
  `note_final_overrider` per overrider), static/deleted/consteval
  override mismatches, `-Woverloaded-virtual` (in `-Wmost`/`-Wall`) and
  `-Winconsistent-missing-override` (on by default), and
  `-Wabstract-final-class`. `-Winconsistent-missing-override` skips a
  method whose name, after following a macro argument to the parameter
  token in the macro body, is spelled in a system header (clang's
  `DiagnoseAbsenceOfOverrideControl`; WP-682,
  `tests/conformance/class.virtual/inconsistent-missing-override-system-macro.cpp`). After the complete-class contexts
  (`act_on_virtual_class_contexts`, from `act_on_finish_class_contexts`):
  overrider exception specifications ([except.spec]/13) and the key
  function (Itanium §5.2.3, stored in `cold.key_function`). The
  exception-specification check is clang's delayed
  `CheckOverridingFunctionExceptionSpec` + `CheckExceptionSpecSubset`
  (WP-171): no check under `-fno-exceptions` or for a destructor of a
  dependent class; otherwise the base's then the overrider's
  specification is resolved (`resolve_exception_spec`: implicit,
  defaulted, and uninstantiated dependent ones), and a still-deferred or
  value-dependent one passes. Tests:
  `tests/conformance/class.virtual/exception-spec-deferred*.cpp`.
- Abstract classes (`abstract_.rs`): `AbstractUse { Return, Param,
  Variable, Field }`, `require_non_abstract(loc, ty, use)`,
  `require_non_abstract_alloc` (new-expression, functional casts),
  `require_non_abstract_element` (arrays), `check_abstract_var`,
  `check_abstract_signature` (definitions only), and
  `recheck_abstract_members` for uses inside the class being defined
  (unknown until `}`); the pure-function notes appear on the first error
  per class (`Sema.abstract_noted`).
- §3.5 graph (`graph.rs`, pure functions of `&Ast`): `pub fn
  subobject_graph(&Ast, record) -> Vec<Subobject>` (numbering as §3.5 and
  `kiln_abi::layout::subobject`, capped at 65535 subobjects), `pub fn
  compute_virtuals(&Ast, record) -> Option<ClassVirtuals>` (None unless
  the class is complete), `pub fn ensure_virtuals(&mut Ast, record) ->
  Option<&ClassVirtuals>` (insert-only cache in `Ast.virtuals`).
  Final overriders: for each (subobject, virtual method) the overriders
  in the subobject's ancestor chain and in every class with the
  subobject's virtual base on its path; overriders dominated by another
  are dropped ([class.virtual]/2). Pure-final-overrider ⇒ abstract.
- `SemaBridge::final_overrider` / `subobjects` (§8.4) compute on demand
  via `ensure_virtuals`; `kiln_codegen/src/fold.rs` still only reads
  `Ast.virtuals` (no sema dependency; WP-079/080).
- §3.1/§3.2: virtual bases are accepted; `lookup_in_class` follows clang
  `lookupInBases` dominance: a result reached through virtual base `V` is
  dropped when another result's class has `V` as a virtual base, and
  results in one virtual base subobject are one result.
  `-Winaccessible-base` (`sema_inaccessible_base`, at the base-specifier)
  for a direct base that is ambiguous as a base of the class.
- §5.8: `act_on_dynamic_cast` (`cast.rs`, clang `CheckDynamicCast`
  order: target kind, complete class or `cv void` pointee, operand kind,
  completeness, casting away constness, same class `NoOp`, upcast
  `DerivedToBase` with access/ambiguity, polymorphic source, `-fno-rtti`
  unless `void*`, else `Dynamic`; reference to a prvalue class operand
  materializes for `&&`); `CastStyle::Dynamic` in `act_on_cast` routes
  there. `typeid` of a glvalue of polymorphic class type is marked
  evaluated (`typeid_expr(.., evaluated, ..)`).
- New diagnostics (`kiln_diag`, end of the tables): the WP-078a ids
  (now sorted into `ids/sema_*.rs`, spec/diag.md §2.4) plus `sema_dynamic_cast_no_rtti`, `sema_inaccessible_base`;
  groups `OverloadedVirtual`, `InconsistentMissingOverride`,
  `AbstractFinalClass`, `InaccessibleBase`.
- Not done (owners): `dtor_delete` (the destructor's operator delete) and
  virtual destructors (WP-067a); deleted/consteval override checks are in
  place but unreachable until `= delete` (WP-067a) and `consteval` members
  land (xfail tests in `class.virtual/`); virtual member templates and
  `virtual` constructors are not diagnosed (no member templates or
  constructors yet); the destructor `-Winconsistent-missing-override`
  variant; `static_cast` downcasts (incl. "via virtual base");
  explicit-object (P0847) virtual functions; overloaded virtual functions
  in one class (WP-068a); copy-initialization of an abstract class (WP-067b).

**Landed (WP-067a).** Module `kiln_sema/src/class/special/{mod,declare,
implicit,deletion,uses}.rs` (§9 says `special.rs`; split by concern).
Constructors, destructors, conversion functions, `= default` and
`= delete` are no longer `sema_unsupported`.
- §4.7 declarations (`declare.rs`): `handles_special_declarator` /
  `act_on_special_declarator` (from decl.rs `act_on_declarator`) build
  `CXXConstructor`/`CXXDestructor`/`CXXConversion`: return types,
  `static`/`virtual`/cv/ref-qualifiers on ctors and dtors, destructor
  name matching (`~X` naming the class, undeclared names), destructor
  and conversion parameters/variadics, copy constructor by value,
  `-Wclass-conversion`, "cannot be redeclared", "definition of
  implicitly declared". `check_explicit_spec` covers `explicit` on every
  declarator (non-function, non-ctor/conv, out of class); `static ~X()`
  drops `static` without invalidating (clang `CheckDestructorDeclarator`).
- §4.1 implicit declaration (`implicit.rs`): `special_member_bits` and
  `finish_special_members` at `}` (`act_on_finish_record_definition`) compute the `NEEDS_*`,
  `TRIVIAL_*`, `IMPLICIT_*_CONST_PARAM` and deleted-ness bits and declare
  the eager members of the §4.1 table; `ensure_implicit(class, kind)`
  declares the rest lazily (the members' own declarator checks and
  §4.2 selection call it). Lookup is `&self`, so a name that can denote an
  implicit member is declared before it is looked up, as in clang's
  `DeclareImplicitMemberFunctionsWithName`. `declare_implicit_named`
  declares copy/move assignment for `operator=` and the destructor for a
  destructor name. It runs in the class of a qualified id-expression and
  in the class of a member access. It also runs in the nominated class of
  a member using-declaration. An unqualified `operator=` runs
  `declare_implicit_named_in_scope` over the enclosing class scopes.
  An implicit member (and its parameters) is in a dependent context
  exactly when its class is ([temp.dep]/1): `declare_implicit` sets
  `dependent_depth` from the class's `DEPENDENT_CTX` around the
  declaration, since the first use of a non-template class may sit in a
  template definition (WP-335). `declare_implicit` sets triviality from the
  class bits, or for copy/move members of a class with `NEEDS_OVL_*` from
  `selected_all_trivial` (clang `SpecialMemberIsTrivial`, §4.3); `constexpr`
  follows the C++23+ rules (§4.6).
- §4.2 deletion (`deletion.rs`): `should_delete` is the Q7 rule over
  bases, fields and variant members, with `select_special` as the
  approximate overload resolution (Found / Missing / Deleted / Ambiguous
  / Unsupported); `deletion_notes` / `deleted_function_notes` give clang's
  `note_deleted_*` explanation. `check_defaulted` / `check_out_of_line_defaulted`
  (§4.4; pending ones run at `}` via `check_pending_defaulted`) check the
  signature (return type, parameter kind, default arguments, declared vs
  implicit type) and emit `-Wdefaulted-function-deleted` with the
  deletion notes and "replace 'default' with 'delete'".
- §4.4 `act_on_defaulted` / `act_on_deleted` (`mod.rs` `defaulted_impl`
  / `deleted_impl`): only special members and comparison operators may be
  defaulted; a deleted definition must be the first declaration; a
  deleted definition is a definition, so a later `= delete` or body is a
  redefinition (also decl.rs `act_on_start_function_def`).
- Uses (`uses.rs`): a named deleted function is queued in
  `special.pending_deleted` and reported at the end of the full
  expression (`act_on_full_expr`), declaration (`act_on_end_abbreviated`)
  or constant evaluation (`eval_const`) as "attempt to use", unless
  `act_on_deleted_callee` saw it called ("call to deleted function",
  "call to [implicitly-]deleted constructor"). §4.8:
  `check_class_default_init` (from `act_on_uninit`) checks the default
  constructor (deleted, inaccessible, const default-init) and the
  destructor (deleted, access) of a variable.
- `define_implicit_special` is the §4.5 stub (`sema_unsupported`
  "implicit definition of a ..."); implicit members are declared but
  have no bodies.
- New diagnostics: the WP-067a ids (now in the `kiln_diag/src/ids/*.rs`
  area files, spec/diag.md §2.4); groups
  `DefaultedFunctionDeleted`, `ClassConversion`. Harness: `//@
  ast-implicit` (harness.md) dumps implicit members.
- Not done (owners): implicit definitions, ctor-initializers and
  constructor calls (WP-067b; so `constexpr C c;` and member-wise copies
  are untested); overload resolution proper (WP-068a: §4.2 selection
  ignores templates and reports Unsupported where they matter, and a
  deleted static member named unqualified inside its class is reported as
  "attempt to use"); deferred and late exception specifications of
  implicit members; the pre-C++23 `constexpr` rules; the `constexpr`-with-virtual-
  base diagnostic (WP-146: constexpr variables and constant evaluation,
§4.10); `explicit` on friends and parameters (WP-068b);
  injected-class-names in the implicit AST dump (xfail
  `ast/class.implicit/eager.cpp`).

**Landed (WP-067b).** Object construction, ctor-initializers, implicit
definitions and temporaries. Modules in `kiln_sema/src/class/`:
- `special/define.rs` (§4.5): `define_implicit_special` builds clang's
  implicit bodies of default/copy/move constructors, copy/move assignment
  (memberwise, trivially assignable arrays via `__builtin_memcpy`,
  `kiln_ast::decl::builtin::MEMCPY`; other arrays by an element-wise
  `for (size_t __iN = 0; __iN != n; ++__iN) to[__iN] = from[__iN]` loop,
  one per dimension, moving the elements of an xvalue source, as clang
  [class.copy.assign]/12.2, WP-202) and
  destructors when odr-used, with the "in implicit ... first required
  here" notes (`Diag::with_context_notes`); `special/excspec.rs` resolves
  implicit exception specifications on use.
- `construct.rs`, `ctor_select.rs` (§5.2, §5.6): `resolve_ctor` selects
  the constructor for direct, copy and list initialization (access,
  deleted, explicit; approximate overload resolution until WP-068a);
  `construct_from_class` for copy/move from the same or a derived class
  (initialization, conversions, arguments, casts); `T()` as
  `CXXTemporaryObjectExpr`; default-initialization builds the default
  constructor call (`init_style` `Call`, `VarSpec::LATE`); P0960
  `CXXParenListInitExpr` for arrays and classes (`ParenList`); brace
  aggregate initialization with bases. `std::initializer_list`
  constructors report `sema_unsupported` (§5.7).
- `ctorinit.rs` (§4.9): mem-initializers are built as their own full
  expressions (`act_on_mem_initializer`), then clang's
  `ActOnMemInitializers` checks, implicit initializers,
  `-Wreorder-ctor`, `-Wuninitialized` in constructors,
  `-Wdangling-field`, and the delegation-cycle check at end of TU.
- `temporary.rs` (§5.3–5.4): `MaterializeTemporaryExpr`,
  `CXXBindTemporaryExpr`, `ExprWithCleanups` at the full expression;
  lifetime extension (`extend_temporaries`: references and the reference
  members of aggregate variables and temporaries) and dangling checks
  (P2748 return, reference member).
- §5.5 NRVO: the `ReturnSlots` model above (`NrvoState`,
  `FnState.returns`, `AstBuilder::clear_return_nrvo`); implicit move.
- §4.8 variable destructors (checked and odr-used); §4.11 the
  `-Wcall-to-pure-virtual-from-ctor-dtor` warning (`member_expr.rs`
  `check_pure_virtual_call_in_cdtor`; `cold.begin_loc` records a pure
  virtual's decl-spec start for its note).
- New diagnostics: the WP-067b ids (now in the `kiln_diag/src/ids/*.rs`
  area files, spec/diag.md §2.4);
  groups `Reorder`/`ReorderCtor`, `Uninitialized`, `Dangling`/
  `DanglingField`/`ReturnStackAddress`, `DelegatingCtorCycles`,
  `CallToPureVirtualFromCtorDtor`.
- Not done (owners): `initializer_list` constructors (follow-up, `construct.rs`); "explicit
  constructor is not a candidate" and no-viable notes, volatile copies
  (WP-068a); class-typed `?:` with different classes, `static_cast` to a
  reference, `new T(args)` for classes (unowned in sema); template
  mem-initializers (WP-070); codegen and Oven of construction (WP-079,
  WP-076); injected-class-names (§2.2).

**Landed (WP-112).** Member exception specifications, constructor
selection, and operator ambiguity:
- *Exception specs*: sema.md §4.7 covers the parser entry point
  `act_on_start_delayed_exception_spec(f) -> bool`, `exc_spec_needed`,
  `resolve_exc_for_type`, `resolve_exception_spec(f, at)` (new `at`
  parameter), lazy instantiation and `FrameKind::ExceptionSpec`.
  `act_on_exception_spec` no longer reports `sema_unsupported`.
  `templ/class.rs` makes `inst_owner` and `spec_pattern_class`
  `pub(crate)` so `except.rs` can find a member's specialization.
- *Constructor templates are candidates* ([over.match.funcs.general]/8):
  `overload/ics.rs` `add_found_template` deduces each constructor or
  member function template against the call's arguments. A
  specialization that would copy its own class by value is not viable
  ([class.copy.ctor]/5; clang `isSpecializationCopyingObject`). It
  fails with `CandidateFailure::IllegalCtor` and the note `candidate
  template ignored: instantiation would take its own class type by
  value` (`note_ovl_candidate_illegal_constructor`).
  `order_template_candidates` runs before the best-viable comparison.
- *Same and derived class construction*: `construct_from_class` now
  calls `resolve_ctor`, which runs full overload resolution
  ([dcl.init.general]/16.6.2.1), instead of a fixed copy/move
  selection. As a result a constructor template can beat the copy
  constructor for a derived-class source. The location used for the
  instantiation note is the entity for direct-init and the expression
  for copy-init, matching clang. "selecting the constructor is not
  supported yet" is gone.
- *Dependent same-class ICS*: `same_dependent_class` ranks a binding
  from the current instantiation (injected-class-name and `X<T...>`,
  [temp.dep.type]/1) as an identity or reference binding. Without this,
  copy and move constructors were ambiguous inside a class template.
- *Constructor template mem-initializers*: `templ/funcs.rs`
  `instantiate_function_spec_definition` replays the pattern's
  mem-initializers, as `templ/class.rs` already did for members of
  class templates.
- *Ambiguous operators*: the 84 `<string_view>` errors came from glibc
  `_ISbit`/`_ISwbit` enumerators being typed as their still-incomplete
  enumeration. [dcl.enum]/5 pre-brace types (`EnumBuild.pre_types`,
  `enumerator_pre_brace_type`) fix them. The ambiguity diagnostics and
  their built-in candidate notes match clang 19
  (`tests/conformance/over.match.oper/ambiguous.cpp`).
- *Bad-conversion notes*: `LosesQuals` reads `no known conversion from
  'A' to 'B'` unless the unqualified types are the same
  (`same_unqualified_referee`), as clang does.
- *Not done*:
  - the AST dump prints `noexcept-unevaluated` where clang prints
    `noexcept-uninstantiated`;
  - the AST dump of a constructor template's pattern has no
    `CXXCtorInitializer`;
  - an outer record named in its own body is dumped as `referenced`,
    where clang marks the injected-class-name instead.

**Landed (WP-082).** Defaulted comparison operators (§4.12):
- *AST* (`kiln_ast`): `class::{CmpCategory, CmpResult, CmpCategoryInfo,
  CmpCategories}` and the field `Ast.cmp_categories` (sema writes each
  category once, when it is validated; the Oven and codegen read it).
  `Exprs::set_call_adl(e)` marks a call whose callee was found only by
  ADL (clang `UsesADL`, dumped `adl`).
- *Sema state*: `special.cmp: CompareState` holds `exclude` (the
  comparison being analyzed or synthesized, and its context),
  `lookups` (the operator lookups captured at each `= default`),
  `default_loc` (where each was defaulted) and `invalid`.
  `OverloadCandidateSet.exclude` keeps that comparison out of its own
  subobject candidate sets. `SynthContext.kind` is now
  `SynthKind::{Special(SpecialKind), Comparison(CmpKind)}`.
- *Entry points* (`pub(crate)`): `defaulted_comparison(f, loc)` (from
  `act_on_defaulted`, which no longer reports `sema_unsupported`),
  `check_defaulted_comparison(f, k, r)`, `declare_implicit_equality`,
  `analyze_cmp`, `define_cmp_on_use(f, loc)`,
  `define_defaulted_comparison(f, use_loc)`, `resolve_cmp_exc`,
  `cmp_category_type`, `cmp_category_of`, `builtin_three_way`.
- *Oven and codegen*: the built-in `<=>` (`BinaryOp::Cmp`) evaluates to
  the category's static member value; aggregates initialized through a
  user-defined conversion are emitted (`agg_visit`).
- *Diagnostics*: the `sema_defaulted_cmp_*` errors,
  `-Wdefaulted-function-deleted` for comparisons, the
  `note_defaulted_cmp_*` notes, `sema_cmp_category_not_found`,
  `sema_std_compare_*`, `sema_spaceship_narrowing_*`,
  `sema_ordered_pointer_zero` and `sema_ovl_deleted_comparison`, all
  with clang 19's text.
- *Gaps* (§11 Q12).

**Landed (WP-456).** Defaulted-comparison correctness (§4.12):
- *AST*: `PseudoObjectExpr` (ast.md §5.3; `AstBuilder::pseudo_object`,
  `syntactic_binary`; `ExprArena::pseudo_syntactic`, `pseudo_semantics`,
  `pseudo_result`).
- *Sema state*: `CompareState.implicit_eq: FxHashMap<DeclId, DeclId>`
  (a defaulted `<=>` -> its implicit `==`).
- *Entry points* (`pub(crate)`): `synth_three_way` (`pub(super)`),
  `defaulted_member_definition(p) -> Option<(DiagId, String)>`.
- *Behaviour*: an objectless qualified member reference (`&A::operator==`)
  resolves the member's exception specification before taking its type
  (`member_ref_without_object`, clang `DiagnoseUseOfDecl`). The overload
  candidate note names an implicit `==` `function (the implicit
  'operator==' for this 'operator<=>)'`.
- *Diagnostics*: `sema_defaulted_cmp_cannot_deduce_undeduced_auto`,
  `note_defaulted_cmp_cannot_deduce_undeduced_auto`,
  `sema_define_explicitly_defaulted_member`; `sema_define_implicit_member`
  gains the `function` select.
- *Tests*: `tests/conformance/class.spaceship/synth-three-way{,-fail}.cpp`,
  `tests/conformance/ast/class.spaceship/synth-three-way.cpp`,
  `tests/codegen/synth_three_way.cpp`,
  `tests/conformance/class.compare.default/{implicit-eq-template,redecl}.cpp`,
  `tests/conformance/expr.prim.id.qual/qualified-operator-{member-access,ast}.cpp`.

**Landed (WP-121): member access by declared name, qualified member
access, out-of-line declarator scope.**
- *Member names* ([expr.ref], [class.dtor]/15, [expr.prim.id.dtor]):
  `x.operator@(...)`, `x.operator T()`, `x.~T()`, `p->~T()`, and
  unqualified `operator@` / `operator T()` in members (implicit `this->`).
  The member name of `CXXDependentScopeMemberExpr` and
  `UnresolvedLookupExpr` is a `DeclName` (`Ast::dependent_member_name`,
  `Ast::unresolved_name`). Entry points take a `DeclName`:
  `act_on_member_dn`, `act_on_id_expression_dn`, `act_on_call_name_dn`,
  `act_on_undeclared_call_name_dn` (the `Symbol` forms wrap them).
  Destructor calls: `act_on_member_destructor`, `rebuild_member_destructor`
  (instantiation). The destructor `MemberExpr` is located at the `~` (clang's
  name start), so a deleted destructor's use is diagnosed there; when
  rebuilt at instantiation it is at the destroyed type, as clang (WP-193).
  Qualified destructor names (WP-223, [basic.lookup.qual.general]/4):
  `act_on_member_destructor_q(base, qual, first, name, ..)`. On a class
  object (`x.N::~Q`; `first` is `None`, `qual` the whole
  nested-name-specifier) `Q` is looked up in `N`, then in the object's
  class, then here, and the destructor of the class it names is accessed
  as a qualified member of `N` through `member_access` (so `N` must be
  the object's class or a base, the call is not virtual, and the errors
  are clang's "no member named '~X' in 'W'" / "'X::~X' is not a member of
  class 'W'"). On a scalar or dependent object
  (`may_be_pseudo_destructor`) the parser splits `p->S::F::~Q` into the
  prefix `S`, the first type-name `F` and `Q`; both are looked up in `S`
  if present, else here, and each must denote the object type
  (`sema_pseudo_dtor_not_type`, `sema_pseudo_dtor_type_mismatch`, second
  name first, as clang). A qualified dependent form is a
  `CXXPseudoDestructorExpr` whose qualified bit
  (`Builder::pseudo_destructor_q`, `Ast::pseudo_destructor_qualified`)
  makes `rebuild_member_destructor(.., qualified, ..)` name a class
  type's destructor as a qualified member of that class (non-virtual,
  and a base class's destructor is allowed: `this->B::~B()`). A
  qualified destructor template-id (`p->N::~X<T>()`) is a gap.
  Inside a class template (its pattern, a partial specialization, or an
  out-of-line member), a destructor name found as the pattern's
  injected-class-name names the injected-class-name type
  (`Sema::injected_named`, [temp.local]/1), so `this->~S()` matches the
  current instantiation's object type at the type check of
  [expr.prim.id.dtor]/2 (WP-336). A non-dependent destructor
  `MemberExpr` in a template (`p->~P()`, `b->B::~B()` with
  non-dependent `p`, `b`) is formed again at instantiation through
  `rebuild_member_destructor` with the destructor's (instantiated)
  class, qualified as written; access is then checked in the
  instantiated member (WP-336).
  Test for the injected-class-name case:
  `tests/conformance/expr.prim.id.dtor/libstdcxx-injected-class-name-dtor-in-template.cpp`
  (libstdc++ shared_ptr_base.h:625, WP-332). A dependent qualifier
  (`this->B<T>::~B()`) is still `templ_unimplemented` ("a dependent
  destructor name").
  A pseudo-destructor on a scalar ([basic.types.general]/9: a pointer
  to member included, `Sema::is_scalar`; libc++'s `std::function` of a
  member function pointer destroys it as `__f_.~_Fp()`; test
  `expr.prim.id.dtor/pseudo-destructor-member-pointer-run.cpp`) or
  dependent object is
  a `CXXPseudoDestructorExpr` (`Builder::pseudo_destructor`,
  `Ast::pseudo_destructor`); the Oven and codegen evaluate its object
  expression only, and the Oven does not end the object's lifetime (as
  for an explicit destructor call; a gap). A pointer to a class is a
  scalar object too ([expr.prim.id.dtor]/2): `p.~P()` with `P` naming
  `S*` is a pseudo-destructor call (libc++'s `__cpp17_destructible` on
  iterator types; WP-339); only when the destructor name denotes the
  pointee (`p.~S()`) does `finish_member_destructor` report clang's
  "member reference type 'S *' is a pointer; did you mean to use '->'?"
  (clang `BuildPseudoDestructorExpr`) and then recovers as clang does: the
  access is taken as `->` and the result is a void pseudo-destructor
  call on the decayed pointer, so `p.~S()` is a well-typed expression
  with no further error of its own (`int a = p.~S();` then reports the
  `void` initializer, as clang).
  The qualified form `p.N::~S()` recovers the same way, after the
  type-name check against the pointee (`act_on_member_destructor_q`;
  WP-352, `tests/conformance/expr.prim.id.dtor/pointer-dot-dtor-arrow-recovery.cpp`).
  *Dependent pseudo-destructor calls* ([expr.call]/5, [temp.dep.expr]/1):
  the call of a `CXXPseudoDestructorExpr` is `void` (`act_on_call`), but
  when the callee is type-dependent (a dependent object or destroyed
  type: `p->~T()`, `t.~T()` in a template) the call is type-dependent,
  as clang's `CallExpr` takes its callee's dependence. kiln gives that
  call the dependent type, so every use of it (`&`, `sizeof`, operators,
  initialization, overload resolution, `decltype`) is formed at
  instantiation, where the rebuilt call is `void`; the AST dump prints
  the dependent call (and a `ParenExpr` around it) as `'void'`, as clang
  (spec/ast-dump.md §3, `CallExpr`). WP-372,
  `tests/conformance/temp.dep.expr/pseudo-dtor-dependent{,-ast,-run}.cpp`.
- *Member not found* ([expr.ref]/7.4, [class.member.lookup]/1): "no
  member named 'm' in 'C'" names the object's class without its
  cv-qualifiers (`member_access`; clang prints the record, so a
  `const It&` object is `in 'It'`; WP-290,
  `expr.ref/no-member-cv-qualified-object.cpp`).
- *Qualified member access* ([expr.ref]/5, [class.member.lookup]):
  `e.N::m`, `p->B::f()`, `this->Base::x`, dependent qualifiers.
  `begin_member_qualifier`/`end_member_qualifier` bracket the qualifier
  (`Sema.member_scope: Option<MemberScope>`: the object's class is looked
  up first, [basic.lookup.qual.general]/2), then `act_on_member_q` /
  `act_on_template_member_q`. `qualifier_names_base` checks the
  qualifier names the object's class or a base
  (`sema_qualified_member_of_unrelated`).
- *Out-of-line declarator scope* ([dcl.meaning]/1, [basic.scope.class]/1;
  clang `ActOnCXXEnterDeclaratorScope`): after a qualified declarator-id
  the parser calls `act_on_enter_declarator_scope(qualifier) -> usize`,
  which pushes the scopes from the qualifier's context to the current
  one (`push_scopes_between`) and sets `Sema.ctx` to the qualifier's
  definition (`Sema.declarator_records`); `act_on_exit_declarator_scope`
  undoes it. `act_on_enter_decl_initializer(v)` does the same for the
  initializer of a qualified variable, and
  `act_on_start_trailing_requires` for a trailing requires-clause (it now
  returns `(usize, bool)`, passed back to `act_on_end_trailing_requires`).
  `this` in such a declarator names the qualifier's class.
  `rebuild_in_current_instantiation(type, declarator)` resolves a
  top-level decl-spec `typename C<X>::m` to the member of the current
  instantiation (clang `RebuildDeclaratorInCurrentInstantiation`); nested
  positions are not rebuilt (a gap).
- *Diagnostics*: the destructor and pseudo-destructor errors
  (`sema_dtor_type_mismatch`, `note_dtor_type_found`,
  `sema_dtor_name_undeclared`, `sema_pseudo_dtor_*`) and
  `sema_qualified_member_of_unrelated`, with clang 19's text.
- *Gaps*: qualified destructor template-ids (`p->N::~X<T>()`); a
  pseudo-destructor qualifier in a dependent scope; dependent
  `operator T()` printing in dumps; the "member has the same name as its
  class" error.
- Tests: `tests/conformance/expr.ref/`, `dcl.meaning/`,
  `ast/expr.ref/`, `class.member.lookup/ambiguous.cpp` (flipped);
  `expr.prim.id.dtor/`, `ast/expr.prim.id.dtor/` (WP-223).

### 8.4 `SemaBridge` (kiln_oven) — public-interface change
Replace `fn vtable(&mut self, record) -> &VTableLayout` with
```
fn final_overrider(&mut self, most_derived: DeclId, at: SubobjectId, method: DeclId)
    -> Option<(DeclId, SubobjectId)>;
fn subobjects(&mut self, record: DeclId) -> &[Subobject];
```
and specify that `ensure_body(f)` defines an implicit or defaulted member on
demand (sema `define_implicit_*`). `kiln_oven::VTableLayout` is deleted; the
Itanium `VTableLayout` lives in `kiln_abi` for codegen. Implementers to update:
`kiln_sema/src/oven_bridge.rs`, `kiln_codegen/src/fold.rs`,
`kiln_oven/tests/common/mod.rs`.

Landed (WP-066a): `kiln_oven` re-exports `kiln_ast::class::{Subobject,
SubobjectId}`; all three bridges read `Ast::virtuals` (keyed by first
declaration), answering `None` / `&[]` until sema fills it (WP-078a).
`kiln_abi::vtable` re-exports the vtable types and declares
`vtable_layout(&Ast, record, &Target) -> Result<VTableLayout,
VTableError>`, `ensure_vtable(&mut Ast, record, &Target) ->
Result<&VTableLayout, VTableError>` (caches in `Ast::vtables`),
`construction_vtable(&Ast, most_derived, base: SubobjectId, &Target)`,
`vtt(&Ast, record, &Target) -> Result<Vec<VttComponent>, VTableError>`
and `dump_vtable_layout(&Ast, record, &Target) -> Result<String,
VTableError>`, with `VTableError { NotDynamic | Incomplete | Unsupported
}`; every body answers `Unsupported` until WP-078b.

Landed (WP-078b): every body is implemented. `VTableError` gains
`TooComplex`, and `Unsupported` became unused (WP-084 removed it). New functions:
- `vtt_vtables(&Ast, record, &Target) -> Result<Vec<SubobjectId>,
  VTableError>`;
- `dump_construction_vtable`;
- `dump_vtable_group`;
- `dump_all_vtable_layouts`.

`kiln_oven` has no `VTableLayout` (nor any vtable reference) in its
sources: the deletion this section asks for has already happened, and
nothing in the Oven reads `kiln_abi::vtable` yet (WP-077).

### 8.5 What codegen reads (WP-079/080)
`RecordLayout` (bases, vbases, vptr, primary base); `VTableLayout` and VTT;
ctor-init lists (complete-object ctor runs virtual-base entries, base-object
ctor skips them; Itanium C1/C2, D0/D1/D2); `CXXConstructExpr` construction
kind; `ExprWithCleanups`/`CXXBindTemporaryExpr` (destroy at end of
full-expression; a bind feeding direct initialization is a no-op);
`MaterializeTemporaryExpr` + `LifetimeExtendedTemporary` (storage, `_ZGR`
mangling index); destructed variables and the `nrvo` bit (NRVO variables live
in the return slot); key function; `dtor_delete`.

## 9. Agent parallelization
| WP | scope (≤ 5 deliverables) | files | needs |
|---|---|---|---|
| WP-066a | AST + interface skeleton: §8.1 types/bits/arenas/builders, dump lines + normalizer rows, sema entry points as `sema_unsupported` stubs, §8.4 trait change | `kiln_ast/*`, `xtask/src/clang_ast/*`, `kiln_sema/src/class/mod.rs` (stubs), `kiln_oven/src/lib.rs` + 3 bridge impls | S3C |
| WP-065 | parser classes (backlog text) against §2.1/§2.7/§8.3 entry points | `kiln_parse/*` | 066a |
| WP-066b | members, `this`, implicit member access, static members, access control, friends, member lookup across bases, derived-to-base checks, pointers to members, definition data | `kiln_sema/src/class/{mod,access}.rs`, `lookup.rs`, `expr.rs` (member access) | 066a, 065 for conformance tests |
| WP-066c | non-virtual-base layout, EBO with bases, tail padding, `record_pack`, dump, differential | `kiln_abi/src/layout/*`, `kiln_abi/tests/*` | 066a (unit tests build ASTs directly); differential needs 065 + 066b |
| WP-067a | implicit declaration (laziness table), deletedness, triviality, `constexpr`, `= default`/`= delete`, ctor/dtor/conversion declaration checks | `kiln_sema/src/class/special/*.rs` (landed as a directory) | 066b |
| WP-067b | construction + ctor-initializers + implicit definitions, temporaries/cleanups/lifetime extension/P2718, elision + NRVO, class init forms + aggregates with bases + paren init | `kiln_sema/src/class/{ctorinit,temporary}.rs`, `init.rs`, `stmt.rs` | 067a |
| WP-078a | virtual functions: overriding, `final`/`override`, covariance, abstract, key function, `dtor_delete`, subobjects + final overriders, `dynamic_cast`/`typeid` sema, virtual bases in sema | `kiln_sema/src/class/virtual.rs` | 066b (067b for virtual-base ctors) |
| WP-078b | virtual layout (primary base, vptr, vbases) + `kiln_abi::vtable` + VTT + vtable dump + both differentials with `virtual` | `kiln_abi/src/{layout/*,vtable.rs}`, tests | 066c, 078a |

Handoffs: 066a merges first (one day); then 065, 066b, 066c run in parallel
(disjoint crates); 067a and 078a run in parallel after 066b (disjoint files);
067b after 067a; S3O's WP-068a after 067b (it calls `materialize_temporary`
and replaces the Q7 approximation); 078b after 066c + 078a. Oven WP-076
needs 067b + 061b; WP-077 needs 078a; codegen WP-079 needs 067b; WP-080 needs 078b. Conflicts to expect:
`lookup.rs`/`expr.rs` (066b only), `init.rs` (067b only), `DefinitionBits`
(066a only — later WPs set, never add).

**Acceptance**: each WP's §10 rows (column "WP") pass under `cargo xtask test`
(harness.md); 066a additionally ships AST unit tests per builder and
dump/normalizer table tests with the existing suite green on stubs; 065 ships
parse-recovery tests and checks the `act_on_*` call sequence on the §10 files
with sema stubs.

## 10. Tests (obligated conformance matrix)
Each directory gets compile-pass, compile-fail (`strict-diags`, every §7
diagnostic of that section at least once) and, where it makes a difference, an
`expect: ast` golden and an Oven `expect: run`/constexpr test once WP-076
lands.

| dir | WP | must cover |
|---|---|---|
| `class.mem` | 066b | every §2.3 member kind, redeclaration errors, complete-class contexts (body, NSDMI, default argument use a later member), NSDMI-needed-early error |
| `class.mfct`, `class.static` | 066b | out-of-line definitions + mismatch; static data members (inline, constexpr, out-of-line, local-class error) |
| `class.this`, `expr.mptr.oper` | 066b | `this` under cv/ref qualifiers, implicit member access, `sizeof(S::x)`, static-function errors, const member assignment, pointers to members |
| `class.access`, `.base`, `.nest`, `class.friend`, `class.protected` | 066b | each [class.access.base]/5 bullet, protected rule, private-inheritance chains, friend functions/classes, hidden friends, nested-class access, delayed checks in out-of-line return types |
| `class.member.lookup`, `class.mi` | 066b, 078a | ambiguity vs static/type/enumerator exceptions, dominance, virtual-base single subobject |
| `class.default.ctor` … `class.dtor`, `class.conv.*`, `dcl.fct.def.default`, `dcl.fct.def.delete`; `ast/class.implicit/` (`implicit` mode, §4.1 table) | 067a | each implicit-declaration rule, each deletion reason (note text), triviality (`__is_trivially_*` after WP-074), constexpr C++20 vs C++23 |
| `class.base.init`, `class.init`, `dcl.init.general`, `dcl.init.aggr` | 067b | order + `-Wreorder-ctor`, delegating, union, missing-init errors, NSDMI interplay, reference member to temporary, aggregates with bases, designated init with bases, paren aggregate init |
| `class.temporary`, `class.copy.elision`, `stmt.ranged`; `ast/` goldens for every §5.1 node | 067b | cleanup shapes, each extension and non-extension context, P2718, the NRVO probes of §5.5 |
| `class.virtual`, `class.abstract`, `expr.dynamic.cast`, `expr.typeid` | 078a | override/final/covariance/consteval mismatch, abstract uses, more than one final overrider, RTTI casts |
| `class.inhctor.init`, `namespace.udecl/class-scope`, `over.match.best/inherited-ctor`; `ast/class.inhctor.init/`; codegen `class_inheriting_ctors`, mangle `inheriting_ctors` | 114 | §4.10: forwarding (by value, references, default arguments, NSDMIs), virtual bases, dependent bases, constexpr (Oven budget) and its notes, the /8 slice, access, /2.7 |
| `kiln_abi` `layout_rules`, `layout_vs_clang`, `vtable_vs_clang` | 066c, 078b | §6.4 (non-virtual in 066c, virtual in 078b) |

Performance and memory (bench.md): `bench/frontend/classes.cpp` (generated: 5k
classes, depth-6 hierarchies, 20 members each, implicit members never used)
gates `Ast::memory()` bytes per class — the lazy policy must keep unused
implicit members at zero bytes — and a reference-workload-style synthetic (many small
polymorphic classes) for `final_overrider`/vtable cache size.

Virtual lattices (WP-400): every walk over a class's bases visits each
class once, never once per path, so a lattice of virtual diamonds (2^N
paths, 3N+1 subobjects) costs polynomial time. The vtable marking
(`class/virtual/used.rs`, clang's recursive `MarkVirtualMembersReferenced`
in one preorder walk with a visited set), the virtual-base list
(`virtual_bases`) and the vbase base-specifier search
(`define.rs` `vbase_spec_index`), the bases-with-fields and repeated-base
checks (`members.rs`), `has_virtual_base` (dominance), and the path
searches of member lookup (`lookup.rs` `class_leaves`) and of
derived-to-base conversion (`conv/bases.rs` `walk`), which skip a class
already found to lead nowhere. In `class_leaves` that memo records the
depth: a search cut by the depth or count limit below a class records the
depth it was reached at, and a later visit at that depth or deeper skips it
(one reached higher is searched again), so a lattice deeper than the depth
limit is not searched once per path either (WP-414). Results and diagnostics are unchanged.
`ClassVirtuals` is cached per class (`ensure_virtuals`) and computed from
the class's own subobject graph. Validation:
`tests/stress/virtual-lattice-deep.cpp` (depth 32),
`bench/frontend/virtual-lattice.cpp` (depth 14 by default, `-DLATTICE_DEPTH=N`;
clang 22 is exponential there). Kiln, release, `-fsyntax-only`: depth 100
0.9 s, depth 200 4.0 s (29 MB) after WP-428 cached the layout of the class
being defined (§6.5; before: 1.9 s, 13.2 s). The remaining cost is the
virtual-base collection (`class/special/implicit.rs` `virtual_bases`, one
walk per query) and the final overriders (`class/virtual/graph.rs`
`all_final_overriders`).

## 11. Open questions (with recommendation)
- **Q1 Implicit-member laziness.** Mirror clang's eager/lazy policy (§4.1;
  needs the append-to-complete-class mutation, §8.2) or declare all six at
  `}` (simpler; ~0.5 KB per class, `implicit` goldens differ)? *Recommend
  clang's policy.*
- **Q2 Final overriders.** Sema computes `Ast.virtuals`; the Oven reads it
  through `SemaBridge::final_overrider` with `SubobjectId` as the shared
  subobject currency (§3.5, §8.4)? *Recommend yes*; the Oven classes WP
  (076) adopts `SubobjectId` for base-class pointer paths.
- **Q3 Default initializer reuse.** clang 19 rebuilds NSDMIs and default
  arguments at every use (`has rewritten init`). S3O §7 already shares
  default arguments, never cloning. *Recommend* the same for NSDMIs: the use
  node carries `used_ctx`; if the stored initializer is an
  `ExprWithCleanups`, the use site sets `cleanup.needs`; `SourceLocExpr` and
  immediate invocations are evaluated against the nearest enclosing
  default-arg/default-init node's location and context (the Oven tracks it).
- **Q4 Explicit object parameters (P0847).** *Resolved* (WP-149) for
  member functions (§2.5), and (WP-356) for lambdas, spec/sema-lambda.md
  §3.1.
- **Q5 Name clash.** `kiln_sema::declspec::BaseSpec` (type-specifier enum,
  re-exported) vs `kiln_ast::BaseSpec` (base-specifier). *Recommend*
  renaming the former to `TypeSpecKind` in WP-066a.
- **Q6 Lookup/access caches** across bases. *Recommend none* in M3; add one
  only if `bench/frontend/classes.cpp` shows it in the profile.
- **Q7 Selection before WP-068a.** Deletedness and constructor selection
  use a unique-candidate approximation (`todo(WP-068a)`); S3O decision 10
  orders 068a after 067, so waiting is not an option. *Recommend the
  approximation*; 068a replaces it and must keep 067's tests green.
- **Q8 `VTableLayout` home.** Delete `kiln_oven::VTableLayout { slots }`
  (no M2 user) and put the Itanium layout in `kiln_abi` (§3.5, §8.4).
  *Recommend yes* — this changes the public `SemaBridge` trait.
- **Q9 WP split.** Replace backlog WP-066/067/078 by 066a/b/c, 067a/b,
  078a/b (§9), with 066a (skeleton) gating everything. *Recommend yes.*
- **Q10 Mutation-table additions** (§8.2), in particular appending implicit
  members to a frozen class and the late `FnSpec::deleted`. *Recommend
  approve*: each is write-once or append-only.
- **Q11 Defaulted comparisons** ([class.compare]: `= default` `operator==`
  and `<=>`, implicit `==` from a defaulted `<=>`) have no owner: S3O §5
  covers only rewritten/reversed candidates. *Recommend* a WP after 067a and
  068b in `class/compare.rs`, reusing §4.2 deletedness and §4.6 constexpr.
  (Done: WP-082, §4.12.)

- **Q12 WP-082 gaps** (owners proposed; none changes an accepted
  program's meaning, each is a missing diagnostic or `not supported yet`):
  - *Resolved (WP-456)*: the synthesized three-way comparison
    ([class.spaceship]/1) is built as a `PseudoObjectExpr` (§4.12), with
    AST, dump, Oven and codegen support.
  - *Resolved (WP-456)*: the implicit `operator==` of a class template's
    defaulted `<=>` is linked to the pattern's (`instantiated_from`) and
    takes the `<=>`'s requires-clause (§4.12 *Templates*).
  - *Resolved (WP-456)*: `&A::operator==` and `a.A::operator==` parse
    (they did since WP-130), and the reference takes the resolved
    `noexcept` type. A direct `DeclRefExpr` still resolves the exception
    spec before the body is defined (`expr.rs` `resolve_exc_for_type`),
    as clang's `DiagnoseUseOfDecl` does.
  - The location of a deduced return type in the `deduced return type ...
    must be 'auto'` error is the type's start column; clang uses the
    `auto` RetLoc.
  - Template candidates whose viability is undecided (WP-071/073
    follow-ups) are assumed usable by the analyzer.
  - Missing beside clang: no `ExplainConstexpr` notes for a non-constexpr
    defaulted comparison; no `-Wtautological-compare`; no
    `ExceptionSpecEvaluation` context note; no consteval or explicit
    object parameter defaulted comparisons (those features are missing).
  - *Resolved (WP-456)*: a redeclaration of a defaulted comparison is
    checked against its resolved exception specification, then for a
    `constexpr` mismatch (§4.12 *Redeclarations*).

### Supervisor decisions (2026-10-28, binding)
- **Q1** clang's eager/lazy policy (goldens must match clang's `implicit` lines).
- **Q2** yes: `Ast.virtuals`, `SemaBridge::final_overrider`, `SubobjectId` is the shared currency (WP-076 adopts it).
- **Q3** yes: NSDMIs and default arguments share the stored initializer with a `used_ctx` payload (Deviation 1 is accepted).
- **Q4** yes: P0847 declarations in 066b, calls in 068a.
- **Q5** yes: rename to `TypeSpecKind` in 066a.
- **Q6** no caches in M3; `bench/frontend/classes.cpp` decides later.
- **Q7** approved: the unique-candidate approximation with `sema_unsupported` (never a different answer); 068a replaces it keeping 067's tests green.
- **Q8** yes: delete `kiln_oven::VTableLayout`; the Itanium layout lives in `kiln_abi::vtable`; the `SemaBridge` trait change is approved (pre-M3, three impls).
- **Q9** yes: BACKLOG rows replaced by 066a/b/c, 067a/b, 078a/b; 066a gates; WP-065 now needs 066a.
- **Q10** approved (write-once / append-only additions to ast.md §2.4).
- **Q11** new **WP-082** defaulted comparisons ([class.compare]) in `class/compare.rs`, after 067a and 068b.
- Inconsistencies 1–11 listed in the handback are assigned: ROADMAP (1) fixed by the supervisor now; types.md (2), sema.md (3), ast.md (4), sema-conv.md (7), the normalizer keys (9), sema-overload.md §7 ownership (10) are fixed by WP-066a in the same PR; (5), (6), (8) are Q5/Q8/Q10.

## Deviations from clang 19
1. `CXXDefaultInitExpr`/`CXXDefaultArgExpr` always share the stored
   initializer instead of rebuilding it per use (Q3, S3O §7); use-site context
   comes from `used_ctx`. The dump is identical.
2. Before WP-068a, implicit-member deletedness and constructor selection use a
   unique-candidate approximation (Q7); programs that need real overload
   resolution there get `sema_unsupported`, never a different answer.
3. What clang mutates lazily in `DefinitionData` (`needs_implicit`, declared
   special members) is sema-side state; the normalizer ignores `definitionData`
   (xtask `clang_ast/table.rs`), so it is unobservable.
4. No `elidable` `CXXConstructExpr`, no pre-C++17 elision modes (kiln is C++20
   and later).
5. Access-check results are not cached (Q6); diagnostics are identical.
6. (WP-066b) Misuse of a non-static member name (`invalid use of
   non-static data member`, "must be called") is diagnosed at the end of
   the enclosing full-expression rather than at the name, so the order of
   diagnostics within one full-expression can differ; where clang emits
   two "reference to non-static member function must be called" errors
   for one misuse, kiln emits one.
7. (WP-066b) `DeclRefExpr`/`MemberExpr` carry no nested-name-specifier;
   `(a.f)()` loses the parentheses when the call is rebuilt. Dumps that
   would show either differ.
8. (WP-066b, WP-174) `&B::m`'s [class.protected] rule and member-pointer
   base-to-derived conversion access are not re-checked; an unqualified
   `&f` naming a member function gets no `must explicitly qualify name of
   member function` diagnostic; calls through member pointers get no
   cv/ref-qualifier check; storage class specifiers on friends are not
   diagnosed.
9. (WP-066b) Out-of-line static data member initializers and the parameter
   types of out-of-line member definitions are not looked up in class
   scope (names before the declarator-id's class are); the body is.
10. (WP-078a) The operand of a potentially-evaluated `typeid` is not
    re-transformed: names in it keep `non_odr_use_unevaluated`, and the
    "expression with side effects will be evaluated" warning is not
    emitted.
11. (WP-078a) The key function does not change when a later out-of-line
    `inline` definition makes the candidate inline (clang recomputes it).
