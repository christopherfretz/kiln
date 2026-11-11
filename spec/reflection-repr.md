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

## 6. Consteval-only ([basic.types.general]/12, [expr.const])

A type is consteval-only if it is `std::meta::info`, or a pointer,
reference, array or member pointer to one, or a function type with one
in its return or parameter types, or a class with a consteval-only base
or non-static data member (cached per complete class). Sema enforces:

- **Variables.** A variable of consteval-only type must be `constexpr`,
  unless it is declared in an immediate function, in an
  immediate-escalating function, or in an immediate context
  (`sema_consteval_only_var`). Checked at the end of its initialization;
  templated declarations are checked per instantiation.
- **Functions.** A function whose type is consteval-only must be
  `consteval`, unless it is immediate-escalating (a constexpr function
  template specialization, a lambda call operator, a defaulted special
  member) (`sema_consteval_only_fn`). A defaulted function is never
  checked. As in GCC 16.1 (CWG3115), the implicit object parameter
  counts: an implicit object member function, constructor, destructor or
  conversion function of a class that is consteval-only by the members
  declared so far (`struct A { int f(); info r; int g(); }` rejects `g`
  only) must be `consteval`; an explicit object parameter
  counts as any parameter; static members are unaffected. Inside a
  template the object-parameter rule is not applied to the pattern.
  A function whose type becomes consteval-only by substitution
  (`template <class T> void h(T)` as `h<info>`, a non-constexpr member
  of `W<info>`) is checked when its definition is instantiated, at its
  name (`check_consteval_only_inst`); a constexpr or defaulted pattern
  escalates instead, and a pattern in a system header is not diagnosed
  (libc++ 19 and libstdc++ 14 predate CWG3115 and instantiate
  non-constexpr helpers for `vector<info>`; kiln extension).
- **Expressions.** A potentially-evaluated expression of consteval-only
  type — a `CXXReflectExpr`, or a reference to a variable, parameter or
  binding of consteval-only type — must be in an immediate context or a
  manifestly constant-evaluated full-expression (a `constexpr`
  variable's initializer, a constant-initialized variable whose
  initializer folds) (`sema_consteval_only_expr`, once per
  full-expression). In an immediate-escalating function the expression
  instead escalates the function to immediate ([expr.const]/18), exactly
  like an immediate invocation (`escalate_consteval_only`; the reason
  note is specified in `spec/consteval-blocks.md` §4). When the variable being initialized is
  itself of consteval-only type, the variable rule reports it, not the
  expression rule. A class member access (`g.s`, `p->m`) whose member
  is of consteval-only type is such an expression even when the object
  expression is not (GCC 16.1 accepts `g.s` and rejects `G::s`; kiln
  rejects both).
- **Structured bindings.** The expression of each binding of a
  decomposition (`e[i]`, `e.m`; spec/sema.md, `struct_bind.rs`
  `decompose_array`/`decompose_members`) is a full-expression checked as
  part of the decomposed variable's initializer: for `static constexpr
  auto [a, b] = arr;` over an `info` array (and the destructuring form
  of an expansion statement over one) the reference to the
  consteval-only `e` is reported by the variable rule, which the
  `constexpr` variable satisfies, not by the expression rule (WP-534).

Codegen emits no block-scope variable of consteval-only type
(spec/codegen.md §4 "Consteval-only variables"; WP-534).

In a template, the leaves are recorded at definition time (where the
evaluation context is known) and re-checked in each instantiation and
each expansion of an expansion statement. A reference to a variable is
such a leaf in a template definition too (WP-1263: `m == b` of two
`constexpr info` variables in a run-time `template for` body, or in a
function template's body, was accepted; GCC 16.1 rejects it the same
way; `meta.reflection.define.aggregate/define-run-template-for-neg.cpp`).
The
codegen backstop (`kiln_codegen/src/consts.rs` "reflection values cannot
be emitted") is reached only through the gaps below.

Known gaps (follow-ups): escalation through an implicit `this->m` leaf
is untested; a defaulted `operator==` of a consteval-only class draws a
spurious `-Wdefaulted-function-deleted` warning.
