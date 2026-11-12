# Reflection representation (§2 of spec/reflection.md)

Status: implemented (WP-472, M6-03), behind `-freflection`. This is the
representation half of the reflection contract: what a `std::meta::info`
value designates, how `^^` builds one for each operand, when two are
equal, how one becomes a template argument, how one prints, and which
types, variables, functions and expressions are consteval-only.
Splices, the query families, `define_aggregate` and annotations are in
spec/reflection.md. References are to P2996R13 and the C++26 working
draft.

Code: `kiln_ast/src/reflect.rs` (`Reflectee`, `ReflectionTable`,
`Ast::reflection_type`), `kiln_parse/src/reflect.rs` (the operand
grammar), `kiln_sema/src/reflect_operand.rs` (`act_on_reflect_*`),
`kiln_sema/src/consteval_only.rs`, `kiln_abi/src/mangle.rs` (the vendor
argument), `kiln_ast/src/dump/names.rs` (printing).
Tests: `tests/conformance/expr.reflect/`,
`tests/conformance/basic.types.general/consteval-only*.cpp`,
`tests/conformance/reflection/`.

## 1. The reflectee set

`ReflectionTable` (spec/ast.md §10) interns `ReflectionId -> Reflectee`.
Every variant is at most 12 bytes (a compile-time assertion), and an id is
a dense 1-based index; the Oven's 8-byte value of a reflection is
`index - 1`, so a value-initialized `info` is the null reflection (id 1,
always present).

| Variant | Designates | Key (what makes two equal) | Dump tag |
|---|---|---|---|
| `Null` | the null reflection | — | `null` |
| `Type(QualType)` | a type | `Ast::reflection_type` (below) | `type` |
| `Decl(DeclId)` | a variable, function, data member, enumerator, structured binding or parameter | first declaration | `decl` |
| `Namespace(DeclId)` | a namespace, the global namespace (the `TranslationUnit` decl) included | first declaration | `namespace` |
| `NamespaceAlias(DeclId)` | a namespace alias itself (`dealias` unwraps it) | first declaration | `namespace-alias` |
| `Template(TemplateNameId)` | a class, variable, function, alias or concept template | canonical `TemplateName` of the first declaration | `template` |
| `Value(ConstValueId)` | a value of structural type (`reflect_constant`) | content-interned store value (it carries its type) | `value` |
| `Object(ConstValueId)` | an object of static storage duration (`reflect_object`) | the interned pointer to it | `object` |
| `BaseSpec(DeclId, u32)` | the `i`-th direct base-specifier of a class | first declaration of the class, index | `base` |
| `DataMemberSpec(DmsId)` | a data member description (`data_member_spec`; spec/reflection.md §6.1) | the content-interned description in the Oven's `DmsTable` (type, name `Symbol`, alignment, width, `no_unique_address`) | `data-member-spec` |
| `Annotation(AnnId)` | an annotation (reserved) | id | `annotation` |
| `FnParam(DeclId, u32)` | the `i`-th parameter of a function (P3096 `parameters_of`, → WP-478) | first declaration of the function, index | `parameter` |

Declaration-keyed variants (`Decl`, `Namespace`, `NamespaceAlias`) are
found through a dense `DeclId index -> ReflectionId` vector (4 B per
declaration up to the highest one reflected) instead of the hash map;
everything else goes through the map. See §3.

**Type key.** `Ast::reflection_type(t)` keeps a top-level alias that
carries no qualifiers of its own: a typedef or alias declaration (keyed on
its first declaration, over the canonical underlying type) or an alias
template specialization (canonical name and arguments). `Elaborated`,
`Using` and paren sugar above the alias are looked through. Everything
else is canonical. So `^^const I == ^^const int`, `^^decltype(1) ==
^^int`, `^^int const == ^^const int`, but `^^I != ^^int` and, for `using J
= I;`, `^^J != ^^I` ([meta.reflection] `dealias` is the only way from one
to the other). GCC 16.1 gives `^^J == ^^I`; kiln follows the paper
(`alias-vs-dealias-identity.cpp`).

## 2. Operands of `^^` ([expr.reflect])

The parser (`parse_reflect_expr`) and sema map the operand forms:

- `^^::` (/4): `Namespace(TU)`.
- `^^type-id` (/6): `Type(reflection_type(t))`. An operand that can be a
  type-id is one (the trailing-return-type disambiguation), unless it is
  a bare template-name.
- `^^namespace-name` / `^^namespace-alias` (/5): `Namespace` /
  `NamespaceAlias`, possibly qualified.
- `^^template-name` not followed by `<` (/5): `Template`. Inside a class
  template, the bare name of the class is its injected-class-name, a
  type ([temp.local]/1).
- `^^template-id` (/6-7): a class or alias template specialization is a
  type; a variable template specialization is `Decl` of the
  specialization; a function template-id is `Decl` of the unique
  specialization it selects without a target ([over.over]). Defaulted
  template arguments count: with `template <class T, class U = long>
  void tf(T, U)` as the only `tf`, `^^tf<int> == ^^tf<int, long>`; two
  viable templates are an overload set. The template-name may be an
  operator-function-id or a literal-operator-id ([temp.names]/1), with
  or without a nested-name-specifier: `^^ns::operator== <int>`,
  `^^operator+<long>`, `^^ns::operator""_x<'1','2'>`,
  `^^S::operator()<int>` reflect the specialization, as GCC 16.1 does
  (`display_string_of` gives `bool ns::operator==(const A&, T) [with T
  = int]`; WP-1246, `expr.reflect/operator-template-ids.cpp`). A
  template-id whose arguments are dependent (`^^ns::plain<T>`,
  `^^operator""_x<C...>`) is a dependent `CXXReflectExpr` (tag 12, `a`
  the `UnresolvedLookupExpr`); substitution re-selects the
  specialization with the substituted arguments.
- `^^id-expression` (/7): `Decl` of the entity it names after lookup — a
  variable (a `constexpr` one reflects the variable, not its value), a
  function, a static or non-static data member, a member function, an
  enumerator, a structured binding, a parameter.
- `^^operator-function-id` / `^^conversion-function-id`, possibly
  qualified (`^^S::operator=`, `^^S::operator()`, `^^S::operator int`,
  `^^S::operator<=>`, a namespace-scope `^^operator+`): `Decl` of the
  function when the name finds exactly one; `has_identifier` is false and
  `identifier_of` fails ("'R' has no identifier"). A conversion-function-id
  that finds no non-template conversion function names the
  specialization of the class's conversion function templates deduced
  for its type ([temp.mem]/6, as `&S::operator int*` and a call
  `S::operator int*()` name it, lookup only in the named class, as
  clang's): with only `template <class T> operator T*()`,
  `^^S::operator int*` is `Decl` of `operator T*<int>` (GCC 16.1
  reflects the template; `expr.reflect/operator-template-ids-kiln.cpp`).
- `^^S::~S` (a destructor name, /7): `Decl` of the
  destructor, the implicit one included (it is declared on demand). The
  type-name after `~` must name S; `^^S::~U` and a pseudo-destructor
  (`^^T::~T` with T scalar, at instantiation) are rejected. A
  destructor, like a constructor, has no type: `type_of` fails ("'R' has
  no type").
- A dependent qualified name — `^^T::x`, `^^T::template m<U>`,
  `^^T::template operator()<int>`,
  `^^typename T::type`, `^^typename T::template m<U>`, `^^ns::W<T>::x`,
  `^^T::operator=`, `^^T::operator int`, `^^T::~T`, nested
  (`^^T::template inner<U>::y`): the parser keeps the qualified-id as a
  `DependentScopeDeclRefExpr` (or the dependent type for the `typename`
  forms) and sema a dependent `CXXReflectExpr` naming it (§2 node
  encoding, tag 12). Substitution resolves it through the ordinary
  dependent-name path (`templ/subst/exprs.rs`
  `reflect_dependent_name`, no string re-lookup) and reflects what the
  qualified-id names: data member, static member, member function (one
  function; several are an overload set), enumerator, member type (only
  with `typename`), member template (only with `template`). Without
  `typename`, a name that instantiates to a type or a class template is
  "missing 'typename' prior to dependent type name 'S::type'"
  ([temp.res.general]/4-5); a missing member is the ordinary "no member
  named" error. A name of the current instantiation (`^^Box::m` inside
  `Box<T>`) reflects the instantiated member, as `^^T` does.
- `^^T` in a template: dependent; the `CXXReflectExpr` stores the
  pattern operand and substitution remaps it (`templ/subst/exprs.rs`),
  so a member of a class template reflects the instantiated member.
  An instantiation-dependent type operand that is not a dependent type
  (`^^ND<T>`, `template <class T> using ND = int;`) is value-dependent
  too ([temp.dep.constexpr]): its reflection is the alias specialization
  of the template's own parameter, so a parameter type `K<^^ND<T>>` is
  substituted (and deduced through the other parameters) rather than
  compared with its definition-time value (`Builder::reflect_expr`,
  WP-1260; `temp.dep.constexpr/reflect-instantiation-dependent-alias-run.cpp`).

Ill-formed operands, each one error:

- an overload set — several functions, several function templates, or
  the constructors of a class (`^^S::S`, [class.qual]/2): "cannot take
  the reflection of an overload set" (also an operator-function-id
  naming several functions, a template-id selecting several
  specializations, and a dependent name instantiating to several
  functions);
- a non-type template parameter: "'^^' cannot be applied to a non-type
  template parameter 'N'";
- anything else — a parenthesized expression, a literal (consumed whole,
  so the rest of the expression still parses): "'^^' cannot be applied
  to this operand";
- an undeclared name: the ordinary undeclared-identifier error.

A base class is never an operand (`BaseSpec` reflections come only from
`bases_of`).

**Node encoding.** A `CXXReflectExpr` of type `std::meta::info` stores
the operand kind tag in `aux` (`Reflectee::tag`: Type 0, Decl 1, Null 2,
Namespace 3, NamespaceAlias 4, Template 5, Value 6, Object 7, BaseSpec 8,
DataMemberSpec 9, Annotation 10, FnParam 11), the one-word payload in `a`,
and in `b` the interned id, or 0 while the operand is dependent
(`VALUE_INST`). A dependent qualified name has tag 12
(`Reflectee::DEPENDENT_NAME_TAG`) with `a` the `ExprId` of its
`DependentScopeDeclRefExpr`; the AST dump prints it as `name dependent`.
A declaration or template operand is dependent when its declaration has
`DEPENDENT_CTX`, except a friend class or class template whose semantic
context is a namespace: first declared by `template <class A, class B>
friend class list;` inside a class template, it is a member of that
namespace ([namespace.memdef]/3) and its reflection is a constant
(`AstBuilder::reflectee_in_dependent_ctx`;
`expr.reflect/friend-class-template-first-decl.cpp`).

## 3. Identity

Two reflections are equal iff their ids are equal ([expr.eq]/6): `==` on
`info` is an id compare, in the Oven and in sema. Interning is the only
place identity is decided, so every key in §1 must map equal entities to
the same `Reflectee`:

- a redeclaration reflects its first declaration, and a friend
  declaration of a function the function it declares: every declaration
  key is `Sema::entity_decl` (spec/sema.md §4 "Entity identity"), so
  `info` equality is `Sema::same_entity`;
- a type, as §1;
- a template, through its canonical name;
- a value or object, by content-interned store value.

Interning a new reflection past the table's limit is the fatal
`sema_too_many_reflections` (`Sema::intern_reflection`; `^^` through
`Sema::build_reflect_expr`), never a panic (WP-471).

`tests/conformance/expr.reflect/equality-matrix.cpp` checks a 29-by-29
matrix of every operand kind (each equal only to itself, and to its
redeclaration or a differently-spelled equal operand); the per-kind files
check the redeclaration and spelling cases.

**Cost.** Interning a declaration reflection is one vector index, and the
lookup verifies the slot's `Reflectee` so that a `Decl` and a
`Namespace` of the same `DeclId` cannot collide. The table is append-only
for the TU; reflections are never freed (they are referenced from frozen
constant values).

## 4. Reflections as template arguments ([temp.arg.nontype], [temp.type])

`std::meta::info` is a structural scalar. A `template <info R>`
argument is frozen as `Scalar::Reflection { ty, id }`, so two arguments
are template-argument-equivalent iff the ids are equal: every reflectee
kind names its own specialization and equal reflections share one
(`nttp-info-kinds.cpp`).

Mangling: until the Itanium ABI defines reflection mangling, kiln emits
GCC 16.1's literal `L Dm <kind> <entity> E` by default (WP-909), the
reflectee's encoding (`_Z1kILDmtyiEEiv` for `k<^^int>`; spec/reflection.md
§8.3 has the grammar, WP-501, WP-879). The string depends on the
reflectee alone, so it is the same in every TU and a specialization keyed
by a reflection takes the reflectee's linkage;
`-fkiln-reflection-mangling=kiln` writes the vendor-extended
`L u6__info … E` instead.

## 5. Printing

The `-fdump-ast` line of a `CXXReflectExpr` is `CXXReflectExpr
'std::meta::info' <kind> <form>` (`<kind> dependent` plus the operand
type, for a `Type`, while the operand is dependent), and notes and
template-argument printing use the same `<form>` (`dump/names.rs`):

| Reflectee | Form |
|---|---|
| `Null` | `std::meta::info()` |
| `Type` | `^^` + the type as kept, its sugar (an alias, a template specialization) printed with its full scope (`^^I`, `^^const int *`, `^^Tm<int>`, `^^const S<int>::In *`): the reflect-expression keeps no qualifier as written |
| global namespace | `^^::` |
| `Decl`, `Namespace`, `NamespaceAlias`, `Template` | `^^` + the qualified name, each class template specialization in its scope followed by its arguments as clang's `printQualifiedName` names a declaration in a diagnostic (`^^ns::f`, `^^S::m`, `^^S<int>::m`, `^^ns::R<char, 3>::Q::y`; WP-1258) |
| `Value` / `Object` | `^^(value of type T)` / `^^(object of type T)` |
| `BaseSpec` | `^^(base #i of C)` |
| `FnParam` | `^^(parameter #i of f)` |
| `DataMemberSpec` / `Annotation` | `^^(data member spec #n)` / `^^(annotation #n)` |

A constant naming a declaration (a template argument `X<&S<int>::m>`, an
"in call to" argument) names it the same way (`PrintEnv::decl_name`).
`display_string_of` follows GCC 16.1 instead (spec/reflection.md §3.9),
which also prints the arguments (`S<int>::m`, `ns::vt<int>`).

The form is stable: tests may match it. Golden:
`tests/conformance/expr.reflect/ast-dump.cpp.expected`.

## 6. Consteval-only values ([expr.const.const], [expr.const.imm]; P4101R1)

Kiln follows the value-based model of P4101R1 (adopted into the working
draft after the Croydon DIS; `docs/STANDARD_SUPPORT.md`): what may not
reach run time is a *value*, never a type. The one predicate lives in
`kiln_ast/src/consteval_only.rs` and is shared by sema and codegen.

- **Consteval-only value** (/1). A reflection other than the null
  reflection; a pointer to an immediate function, or to or past the end
  of (a subobject of) an immediate object; a pointer to member naming an
  immediate member function. `Ast::has_consteval_only_value(c)` walks a
  frozen value: scalars directly, aggregates through their relocations
  (every pointer, member pointer and reflection subobject), pointees
  through the variable, temporary or template parameter object they
  name (`Ast::object_value`), with a seen-set for self-referential
  constants.
- **Immediate object** (/2). An object whose complete object has a
  consteval-only constituent value or a constituent reference to an
  immediate object or function: `Ast::is_immediate_var(d)` for a
  variable or template parameter object `d` whose value is known. A
  variable not (yet) evaluated is not one. The null reflection is not
  consteval-only: `std::meta::info x;`, `S s;` of a class with an `info`
  member, `info* p = nullptr;` and `constexpr S s{{}, "x"}` are not
  immediate objects.
- **Variables** (/3). Every immediate object must be a `constexpr`
  variable's object, a template parameter object, or live within a
  constant evaluation. A non-`constexpr` variable whose initializer held a
  consteval-only leaf is allowed only when its initialization is constant
  (the initializer is manifestly constant-evaluated); its folded value is
  then checked and, if consteval-only, it is an error
  (`sema_consteval_only_var`, "'v' is initialized with a consteval-only
  value but is not declared 'constexpr'"; `check_consteval_only_var`). A
  reference is not an object: a non-`constexpr` reference bound to an
  immediate object is well-formed, and its value is recorded so that each
  use of it is a leaf. A variable whose initialization is not constant
  reports the leaf instead (the expression rule). The rule applies at
  namespace and block scope, to static data members, structured bindings,
  expansion statement variables and template instantiations alike; all
  reach the same `check_consteval_only_var`.
- **odr-use before the definition.** "If an immediate object is odr-used
  ... reachable from a point where it is not defined, the program is
  ill-formed": within one translation unit an earlier use is always
  reachable from the later definition, so the diagnostic is required. A
  reference to a static-storage variable none of whose declarations has an
  initializer is remembered (`note_early_odr_use`, keyed by the first
  declaration) and reported, with a "declared here" note, when the
  definition is evaluated and found immediate (`report_early_odr_uses`).
- **Expressions** (/5, [expr.const.imm]/2.1-3). An *immediate constant
  expression* is a glvalue referring to an immediate object or a prvalue
  whose result object is one. Sema tracks it through its *leaves*: a
  `CXXReflectExpr`; a reference (`DeclRefExpr`, `MemberExpr` of a static
  member) to an immediate variable, binding (through its decomposed
  variable) or template parameter object; a substituted non-type template
  argument whose value is consteval-only; an immediate invocation whose
  result is consteval-only; a default argument or default member
  initializer that holds a leaf, at each use. Each rides the
  immediate-reference list of its full-expression and is classified with
  the immediate invocations: inside an immediate invocation or an
  immediate function context it is fine; in a manifestly
  constant-evaluated initializer it is fine (then the variable rule
  applies); in a full-expression that is itself a constant-expression
  (a template argument, `static_assert`, an array bound, `case`,
  `noexcept`, a requires-clause: one finished by
  `act_on_constant_full_expr`) it is fine (/1.2). The parser enters that
  context before building the expression, so its invocations are not
  even recorded; template instantiation rebuilds it outside that
  context, so the leaf check itself asks (`in_constant_full_expr`), and
  both spellings get one answer. In an immediate-escalating function it makes the function
  immediate (/2.1 read literally: even an immediate constant expression
  that is itself a constant expression escalates; `escalate_consteval_only`,
  reason note in `spec/consteval-blocks.md` §4); elsewhere it is an error
  (`sema_consteval_only_expr`, "consteval-only value outside an immediate
  function context", once per full-expression). A default member
  initializer used by a constructor is part of its body
  ([expr.const.imm]/4 Note 1): an implicit (escalating) constructor
  escalates, any other is an error at the leaf.
- **No permitted-result restriction** (/4 struck it). Within land 1 the
  old rejection of a `constexpr` variable holding a pointer to an
  immediate function is kept (land 2 lifts it with P3603).
- **Templates.** Leaves are recorded at definition time where the
  evaluation context is known (`consteval_only_pattern`) and re-decided by
  value in each instantiation and each expansion of an expansion
  statement (`note_inst_consteval_only`, `note_shared_consteval_only`,
  `leaf_is_immediate`). A candidate whose value is unknown in the
  definition — a `constexpr` variable or a binding of a `constexpr`
  decomposition of dependent type or initializer, a non-type template
  parameter that can hold a pointer or a reflection — is kept only when
  its substituted value is consteval-only (`is_pending_constexpr_leaf`).
  The element variable of a `template for` over a class with an `info`
  member is therefore a leaf exactly when the element's value holds a
  non-null reflection.

The P2996R13 type walk (`is_consteval_only_type`) survives for one kiln
restriction only: a function whose type holds `std::meta::info` must be
`consteval` (or immediate-escalating), as before, including the CWG3115
implicit-object-parameter rule and the system-header exemption for
instantiated non-constexpr helpers (`sema_consteval_only_fn`,
`check_consteval_only_inst`). P4101R1 allows such functions with a
run-time null `info`; that needs a run-time representation and is land 2
of WP-1328.

Codegen decides by value too (spec/codegen.md §4 "Consteval-only
variables"): an immediate object has no storage; a variable whose type
reaches `std::meta::info` but whose value is not consteval-only has none
either in land 1 — a `constexpr` one is read only through its folded
members, any other is the error "code generation for a run-time
'std::meta::info' value is not supported yet", as is a null reflection
constant reaching emission.

Known gaps (follow-ups): a defaulted `operator==` of a class with an
`info` member draws a spurious `-Wdefaulted-function-deleted` warning.
