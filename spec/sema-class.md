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
| special members | all six: implicit declaration/definition, triviality, deletedness, `constexpr`, `= default`/`= delete`, inheriting ctors (after WP-062) | defaulted comparisons ([class.compare]; owner to be assigned, see §4.4), trivially relocatable (P2786) |
| construction | ctor-initializers, delegating ctors, NSDMI, temporaries, cleanups, lifetime extension incl. P2718 range-for, guaranteed elision, NRVO marking, aggregates with bases, paren-aggregate init | function-try-blocks and EH cleanups (exceptions milestone) |
| layout | full Itanium: bases, vptr, vbases, primary base, EBO, tail padding, bit-fields across bases, `#pragma pack`, `alignas`; vtable layout and VTT | MS ABI, `#pragma ms_struct`, `-fclang-abi-compat` |
| RTTI | `dynamic_cast`/`typeid` sema; final overriders for the Oven | |
| other | | explicit object parameters (P0847) beyond declaration (Q4) |

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
   `public`).
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
| static data member | `Var`, `VarStorage::Static`, ctx = class | in-class: a declaration unless `inline` or `constexpr` (implicitly inline, [dcl.constexpr]/1); a `static const` integral/enum member with an initializer is usable in constant expressions; `int S::s = 1;` is a redeclaration (lexical ctx = namespace, `HAS_LEXICAL_CTX`) |
| member function | `CXXMethod` | in-class definition implicitly inline; `static` → `FnStorage::Static`; cv/ref-qualifiers into `FuncProto.this_quals`/`ref_qual` (types.md §5.4); `virtual`, `= 0`, `override`, `final` (§3.4) |
| constructor / destructor / conversion | `CXXConstructor` / `CXXDestructor` / `CXXConversion` | `DeclName` tag 2/3/4 keyed by the canonical class `TypeId` / target `QualType` (ast.md §3) |
| nested class / enum / typedef / alias | M2 kinds, ctx = class | access applies to the name; nested class is a member ([class.access.nest]) |
| friend | `Friend` (payload: friend decl or type) | §2.4; a friend function first declared in a class is a member of the innermost enclosing namespace, hidden from ordinary lookup ([namespace.memdef]/3, [class.friend]/7): it is entered in that namespace's lookup table with `LookupFlags::ADL_ONLY` (sema-overload.md §2.2, decision 7), which ordinary lookup skips and ADL (WP-068a) finds; no side table. A later namespace-scope redeclaration is entered without the flag and is visible to ordinary lookup |
| access-specifier | `AccessSpec` | changes `ClassBuild.access`; kept for the dump |
| `using` member | WP-062 (`Using`, `UsingShadow`) | shadow access = the using-declaration's; inheriting constructors §4.10 |
| member template | WP-070 | |

Per-declaration checks: redeclaration of a member (`class
member cannot be redeclared`, `constructor cannot be redeclared`, `duplicate
member 'a'`); `mutable` with `const` or a reference; `constexpr` non-static
data member; incomplete or abstract field type (`field has incomplete type
'In'` + note `definition of 'In' is not complete until the closing '}'`);
static data member in a local class or in an anonymous/unnamed class;
`virtual`/`explicit`/pure on the wrong kind of member.

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
of `X` and their members ([class.local]/2). [class.protected]: for a non-static
protected member named through an object expression or `&P::m`, when only (c)
via `P` grants access the object's class must be `P` or derived from `P` (`'p'
is a protected member of 'S'` + note `can only access this member on an object
of type 'D'`). Private: `'x' is a private member of 'S'` + `declared private
here` / `implicitly declared private here`.

**Checked**: every name found by class member or qualified lookup into a class
(for functions, the one selected by overload resolution); selected
constructors, destructors and conversion functions; subobject members used by
implicit definitions (failure deletes the member, §4.2); derived-to-base
conversions (§3.3); `operator delete` of virtual dtors. Overriding ignores
access.

**Delayed checks.** Checks raised while parsing decl-specifiers and the
declarator queue in `Sema.delayed_access` and resolve when the declaration
exists, in the [class.access]/7 context: for `S::B S::f(S::B)` the whole
declaration including the return type is checked as a member of `S`; for a
friend declaration, as the befriending class. In template patterns: at
instantiation (WP-070); in SFINAE contexts: substitution failure (S3T). No
result cache in M3 (Q6).

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
  object argument`.
- In a `const` member function, members through `this` are const unless
  `mutable` ([expr.ref]/6): `cannot assign to non-static data member within
  const member function 'f'` + `member function 'K::f' is declared const here`.
- `implicit_object_type(method)` gives `cv C&` / `cv C&&`
  ([over.match.funcs.general]/4) for S3O's ranking.
- Explicit object parameters (P0847): declared (FnSpec `explicit_object`),
  calls `sema_unsupported` in M3 (Q4).

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
declaration ([dcl.meaning.general]/2). Tests: `expr.eq/`,
`dcl.mptr/template-id-qualifier.cpp`, `basic.lookup.qual/not-a-scope.cpp`,
`dcl.meaning/qualified-no-match.cpp`.

### 2.7 Complete-class contexts ([class.mem]/7)
The parser (WP-065) saves the tokens of member function bodies (with
ctor-initializers), default arguments, *noexcept-specifier*s, NSDMIs and
contract assertions. Until replay: parameters carry `ParamData` flag
`DEFAULT_ARG_PENDING`, methods have `ExcSpec::Deferred(self)` and sit in
`Sema.unparsed_exc`, fields have NSDMI style bits but `init == None`. After the
outermost class's `}` the parser replays in clang's order
(`ParseLexedMethodDeclarations`, `ParseLexedMemberInitializers`,
`ParseLexedMethodDefs`), each phase over the class and its nested classes in
declaration order:
1. default arguments → `act_on_param_default_arg(p, e)` (write-once),
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
  it only when the initializer parsed.

Per outermost class the parser calls, after
`act_on_finish_record_definition(r, _)`: for each phase in order,
`act_on_start_delayed_class(r)`, the phase's items in declaration order
(a nested class's items wrapped in its own
`act_on_start/finish_delayed_class`), `act_on_finish_delayed_class(r)`;
then `act_on_finish_class_contexts(r)`. The pinned sequences are the
trace tests in `kiln_parse/src/class.rs` (parse.md §10).

Needing an unparsed NSDMI earlier (a nested class's implicit default ctor used
in the enclosing class) is an error: `default member initializer for 'x'
needed within definition of enclosing class 'S' outside of member functions`.

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
differs. Which implicit members have been declared is sema-side state
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
- **consteval** mismatch either way: `consteval function 'f' cannot override a
  non-consteval function` (and converse).
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
  ([class.dtor]/16) into cold map `dtor_delete`.
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
dtor. Until WP-068a lands, selection uses the unique non-template candidate with
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
copy ctor is selected); an
`= default` on first declaration gets it at `}` (`mark_function`, already
`LATE`). Eligibility ([special]/6) of constrained special members is WP-073's;
for non-template classes every non-deleted declared special member is eligible.

### 4.4 `= default`, `= delete`, user-provided
`act_on_defaulted(f, loc)` / `act_on_deleted(f, loc, msg)` (C++26 message in
the cold map). Defaultable: special members and comparison operators
(defaulted comparisons, [class.compare], are covered neither by S3O §5 nor
here; until a WP owns them they are `sema_unsupported`); else `only special
member functions and comparison operators may be defaulted`. Explicitly defaulted on its first
declaration → not user-provided; out of line → user-provided, defined there. A
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
not `constexpr` when a member's selected ctor is not). `HAS_CONSTEXPR_CTOR` and
`LITERAL` are computed from these. `DEFAULTED_DEFAULT_CTOR_CONSTEXPR` records
the default ctor's answer before it is declared.
WP-067a implements the C++23+ rule only; the C++20 rule is open.

### 4.7 Constructors, destructors, conversion functions (declarations)
Constructor: no return type (`constructor cannot have a return type`), not
`static`/`virtual`/cv-/ref-qualified, copy ctor by reference (`copy constructor
must pass its first argument by reference`), `explicit` / `explicit(bool)`
(cold map). A non-explicit constructor is converting ([class.conv.ctor]).
Destructor: name matches the class, no parameters (`destructor cannot have any
parameters`), no return type; implicitly `noexcept` unless a subobject's isn't
([except.spec]/9). Conversion function ([class.conv.fct]): no return type
(`conversion function cannot have a return type`), no parameters, `DeclName`
tag 4 on the canonical target; conversion to the class itself, a base or `void`
warns [-Wclass-conversion]. `explicit` elsewhere: `'explicit' can only be
applied to a constructor or conversion function`. Selection (converting ctors
and conversion functions in user-defined conversion sequences) is S3O's.

### 4.8 Destruction of variables
Sema does not insert destructor calls for named objects: a variable, parameter
or `LifetimeExtendedTemporary` whose type is destructed ([basic.life], clang
`isDestructedType`) is destroyed by the Oven/codegen at scope exit in reverse
declaration order. Sema's obligation when such a variable is declared: the
destructor is accessible (`variable of type 'X' has private destructor`) and
not deleted (`attempt to use a deleted function`), and is marked referenced
(implicit definition).

### 4.9 Ctor-initializers ([class.base.init])
`act_on_mem_initializers(ctor, &[MemInitInfo], colon_loc)`, `MemInitInfo { id:
Ident | TypeId, loc, init, pack_expansion }`. A mem-initializer-id names a
non-static data member (anonymous-union members via `IndirectField`), else a
direct or virtual base, or the class itself (delegating). Errors: `member
initializer 'y' does not name a non-static data member or base class`,
`multiple initializations given for non-static member 'x'` (`... for base
'B'`), `an initializer for a delegating constructor must appear alone`,
`initializing multiple members of union`.

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
constructor`. Each written initializer is a full-expression. `reference member
'r' binds to a temporary object whose lifetime would be shorter than the
lifetime of the constructed object` + `reference member declared here`.
Warnings: `field 'b' will be initialized after field 'a'` [-Wreorder-ctor],
`field 'b' is uninitialized when used here` [-Wuninitialized]; delegation
cycles at TU end: `constructor for 'S' creates a delegation cycle`
[-Wdelegating-ctor-cycles].

**Storage**: `CtorInit { target: u32 (field DeclId or base TypeId), kind: u8
(Base | VirtualBase | Field | IndirectField | Delegating), written_index: u8 (0
= implicit), init: ExprId, loc }` (16 B) in arena `Ast.ctor_inits`, cold map
ctor → `ListId`, written once with the body. Dump `CXXCtorInitializer 'S'` /
`CXXCtorInitializer Field 'm' 'int'`; JSON `baseInit` / `anyInit` /
`delegatingInit`.

### 4.10 Inheriting constructors ([namespace.udecl]/3, [class.inhctor.init])
After WP-062: `using B::B;` creates `ConstructorUsingShadow` decls. Overload
resolution (S3O) considers the shadowed ctors; a selected one gets an implicit
`CXXConstructor` in the derived class (clang `findInheritingConstructor`) whose
body initializes the base with `CXXInheritedCtorInitExpr` and the remaining
subobjects as a defaulted default ctor would.

### 4.11 During construction and destruction ([class.cdtor])
Sema warns on a direct call of a pure virtual function from a ctor/dtor of its
class: `call to pure virtual member function 'f' has undefined behavior;
overrides of 'f' in subclasses are not available in the constructor of 'C'`
(+ `'f' declared here`). Dynamic-type rules during construction are the
Oven's and codegen's (WP-076/077, vptr stores in ctors).

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
of a reference to a temporary (`returning reference to local temporary object`
[-Wreturn-stack-address]), a mem-initializer reference member (§4.9), a
temporary in a `new`-initializer or a function argument (full-expression
lifetime).

**P2718R0 (C++23 range-for)**: temporaries in the *for-range-initializer* are
extended to `__range` (clang 19 dump: `MaterializeTemporaryExpr 'S' xvalue
extended by Var '__range1'`). Sema collects every full-expression-duration
`MaterializeTemporaryExpr` created while parsing the range-initializer
(`Sema.extend_collector: Option<Vec<ExprId>>`, clang
`InLifetimeExtendingContext`), including those in default arguments of calls in
it, and extends them all to `__range`.

Status (WP-067b): not implemented. Range-based `for` itself (parser,
`CXXForRangeStmt` builder, dump, codegen, Oven) does not exist yet and no
work package owns it; `Sema.extend_collector` is declared but unused, so
P2718 and the `stmt.ranged` conformance directory are blocked on it.

### 5.5 Copy elision and NRVO ([class.copy.elision])
- **Guaranteed elision** ([dcl.init.general]/16.6.1): a prvalue of the same
  class (ignoring cv) initializes the object directly; no copy or move
  `CXXConstructExpr` is formed.
- **Implicit move** (C++23 [expr.prim.id.unqual]/4): in `return` and
  `co_return` operands an id-expression naming an implicitly movable entity is
  an xvalue (clang 19 dump: `ImplicitCastExpr 'const S' xvalue <NoOp>` before
  binding to `const S&`).
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
| copy-init from `T`/derived | elision for prvalue `T`; else copy/move ctor (derived glvalue: `DerivedToBase` then ctor) |
| copy-init from other type | user-defined conversion (S3O): `ConstructorConversion` around `CXXConstructExpr`, or `UserDefinedConversion` around the conversion call; `explicit` ctors excluded (`no viable conversion from 'int' to 'T4'` + candidate notes incl. `explicit constructor is not a candidate`) |
| list-init, aggregate | `InitListExpr`: C++17 base elements first (each initialized from its element, brace elision applies), then fields; designated initializers name direct members only, bases are then initialized from `{}` |
| list-init, non-aggregate | [over.match.list] two phases (initializer-list ctors, then all ctors); `CXXConstructExpr` with `list`; narrowing as M2 |
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
  parameter-type-list, cv- and ref-qualifiers), implicit virtualness,
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
  `-Wabstract-final-class`. After the complete-class contexts
  (`act_on_virtual_class_contexts`, from `act_on_finish_class_contexts`):
  overrider exception specifications ([except.spec]/13) and the key
  function (Itanium §5.2.3, stored in `cold.key_function`).
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
- New diagnostics (`kiln_diag`, end of the tables): the WP-078a block in
  `ids.rs` plus `sema_dynamic_cast_no_rtti`, `sema_inaccessible_base`;
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
  §4.2 selection call it). `declare_implicit` sets triviality from the
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
- New diagnostics: the WP-067a block in `kiln_diag/src/ids.rs`; groups
  `DefaultedFunctionDeleted`, `ClassConversion`. Harness: `//@
  ast-implicit` (harness.md) dumps implicit members.
- Not done (owners): implicit definitions, ctor-initializers and
  constructor calls (WP-067b; so `constexpr C c;` and member-wise copies
  are untested); overload resolution proper (WP-068a: §4.2 selection
  ignores templates and reports Unsupported where they matter, and a
  deleted static member named unqualified inside its class is reported as
  "attempt to use"); deferred and late exception specifications of
  implicit members; the pre-C++23 `constexpr` rules; the operator delete
  of a virtual destructor ([class.dtor]/16); the `constexpr`-with-virtual-
  base diagnostic; `explicit` on friends and parameters (WP-068b);
  injected-class-names in the implicit AST dump (xfail
  `ast/class.implicit/eager.cpp`).

**Landed (WP-067b).** Object construction, ctor-initializers, implicit
definitions and temporaries. Modules in `kiln_sema/src/class/`:
- `special/define.rs` (§4.5): `define_implicit_special` builds clang's
  implicit bodies of default/copy/move constructors, copy/move assignment
  (memberwise, arrays via `__builtin_memcpy`, `kiln_ast::decl::builtin::MEMCPY`) and
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
- New diagnostics: the WP-067b block at the end of `kiln_diag/src/ids.rs`;
  groups `Reorder`/`ReorderCtor`, `Uninitialized`, `Dangling`/
  `DanglingField`/`ReturnStackAddress`, `DelegatingCtorCycles`,
  `CallToPureVirtualFromCtorDtor`.
- Not done (owners): P2718 and range-based `for` (unowned, §5.4);
  `initializer_list` constructors (follow-up, `construct.rs`); "explicit
  constructor is not a candidate" and no-viable notes, volatile copies
  (WP-068a); class-typed `?:` with different classes, `static_cast` to a
  reference, `new T(args)` for classes (unowned in sema); template
  mem-initializers (WP-070); codegen and Oven of construction (WP-079,
  WP-076); injected-class-names (§2.2).

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
| `kiln_abi` `layout_rules`, `layout_vs_clang`, `vtable_vs_clang` | 066c, 078b | §6.4 (non-virtual in 066c, virtual in 078b) |

Performance and memory (bench.md): `bench/frontend/classes.cpp` (generated: 5k
classes, depth-6 hierarchies, 20 members each, implicit members never used)
gates `Ast::memory()` bytes per class — the lazy policy must keep unused
implicit members at zero bytes — and a mirrorbind-style synthetic (many small
polymorphic classes) for `final_overrider`/vtable cache size.

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
- **Q4 Explicit object parameters (P0847).** *Recommend* declarations in
  WP-066b, calls in WP-068a (S3O ranks the explicit object parameter).
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
8. (WP-066b) Access of type names found by qualified lookup (`A::In`) is
   not checked; `&B::m`'s [class.protected] rule and member-pointer
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
