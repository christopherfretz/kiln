# Sema: templates, deduction, packs, concepts

Status: **design** (WP-S3T). This spec is normative for WP-069 to WP-074
and for the template parts of WP-075 and WP-081. It covers the parser
side (`kiln_parse`), the semantic side (`kiln_sema::templ`) and the AST
tables (`kiln_ast::templates`).

Related specs:
- ast.md §2.4, §5, §8 and §16.14: AST shapes, `TemplLink`/`SpecInfo`, and
  the instantiation meter.
- types.md §5.3, §7.2, §8 and §17–18: template types, `ProfileId`,
  `TemplateArg`, and the approved decisions T1/T3/T6.
- oven.md: `SemaBridge`, `EvalContext`, budgets.
- sema.md and parse.md: the M2 baseline that this spec extends.
- sema-overload.md (S3O) and sema-class.md (S3C): candidate sets,
  completeness, and class properties.

Oracle: where the standard leaves a choice, kiln follows clang 19. Every
deviation is listed in §1.5.

## 1. Scope, principles and performance goals

### 1.1 Scope by milestone

| Area | M3 (this spec, WP-069..074) | Later |
|---|---|---|
| Declarations | class, function, variable and alias templates; member templates; concepts; abbreviated function templates; friend templates | `export` and modules reachability (M4) |
| Specialization | implicit instantiation, partial and explicit specialization, explicit instantiation (declaration and definition) | — |
| Deduction | calls, explicit args, defaults, address of overload set, conversion functions, `auto`/`decltype(auto)`, NTTP `auto`, partial ordering | deducing `this` (M4) |
| CTAD | implicit guides from constructors, the copy guide, user-declared namespace-scope guides | aggregate CTAD (P1816), alias CTAD (P1814), class-scope guides of member templates, inherited-ctor CTAD (P2582) (all M4) |
| Packs | type, value and template packs; expansion in every context in §2.7; folds; `sizeof...`; pack indexing P2662 | structured-binding packs P1061 (clang 19 has no support, M4); variadic friends P2893 (M4); expansion statements P1306 (M6, with reflection) |
| Concepts | normalization, satisfaction, subsumption, `requires`-expressions, constrained `auto` | fold-expanded constraints P2963 (clang 19 has no support, M4); concept and variable-template template parameters P2841 (M5) |
| Reflection | — | splices as template arguments (M6) |

Concepts are in M3 because the exit headers need them: `<span>`,
`<string_view>`, `__concepts/*` and the iterator concepts that `<array>`
pulls in (§13.3). ROADMAP.md puts them in M4; see Q1.

### 1.2 Principles

P1 **Lazy and memoized.**
- Each specialization is created at most once. It is keyed by `(first
  DeclId of the template, canonical ArgListId)` in
  `TemplateTables::specializations` (ast.md §8.3). The key is 8 bytes and
  is found with one hash probe.
- Class instantiation creates member *declarations* only.
- Each definition (member function, static data member, member class,
  default argument, NSDMI, exception spec) is instantiated separately, at
  the first use that needs it (§4.6).
- Failed deductions leave nothing in the AST (ast.md §8.4).

P2 **No re-parsing.**
- No template keeps tokens.
- The pattern is an ordinary AST with dependent nodes.
- Substitution (§5) is a tree transform. It builds *fresh* nodes through
  the same `Sema` actions the parser uses.
- Pattern nodes are never mutated (ast.md §2.4).

P3 **Accounting is built in.**
- Every instantiation runs inside an `InstantiationMeter` frame (ast.md
  §16.14), extended with self and inclusive nodes, bytes and time (§11.3).
- A per-TU instantiation table, like `-ftime-report`, is available as text
  and as JSON.
- The `instantiation.retained` MemReport row is mandatory.

P4 **Bounded native stack.**
- Instantiation is re-entrant by nature: a body needs a complete class,
  whose static member needs a constant, whose evaluation needs a body.
- Kiln bounds this recursion three ways:
  - (a) The `-ftemplate-depth` check on every instantiation frame
    (default 1024, as clang).
  - (b) Sema runs on a dedicated thread whose stack is sized for that
    depth.
  - (c) A `StackGuard` measures the remaining stack (in safe Rust) at every
    frame push and every 16th substitution step. It turns overflow into a
    fatal diagnostic, never a crash (§10.3, Q4).
  - *Landed (WP-086):* the check is two address reads and a compare, so it
    runs at **every** substitution step (`Substituter::expr`, `ty`,
    `stmt`), not every 16th, as well as at every frame push, at the
    parser's recursive rules and at each Oven entry from sema (§10.3).
- End-of-TU instantiation is a worklist (`pending`), not recursion.

P5 **Diagnostics identical to clang 19.**
- This covers note texts, note order, elision, and when the instantiation
  stack is printed (§10).
- Compile-fail goldens compare them exactly.

P6 **Canonical identity, sugared display.**
- Specialization keys use canonical arguments.
- Types keep `SubstTemplateTypeParm` and `TemplateSpecialization` sugar for
  printing (types.md T3).

P7 **Each failure is memoized.**
- A specialization whose instantiation failed is marked `invalid` and is
  never retried. Its errors are reported once.
- An unsatisfied constraint is cached like a satisfied one.

### 1.3 Performance goals

These are gated in `bench/frontend` (§11.4).

| Metric | M3 exit | M7 |
|---|---|---|
| Implicit instantiations per TU | ≤ clang's count on the same TU, from `-ftime-trace` `Instantiate*` events (laziness parity) | strictly fewer where kiln is lazier (§3.3 sharing) |
| `templates.cpp` wall time (`-fsyntax-only`) | ≤ 1.5× clang | ≤ 0.7× clang |
| Retained AST bytes per class specialization (declarations only) | ≤ 64 B + 24 B per member declaration | as M3 |
| Specialization lookup | 1 hash probe and 0 allocations on a hit | — |
| Satisfaction of a repeated `(concept, args)` | 1 hash probe | — |
| Peak RSS on the M3 exit headers | ≤ clang | ≤ 0.5× clang |
| Instantiation depth 1024 | no stack overflow; the fatal diagnostic matches clang | — |

### 1.4 Not in this spec

These are owned elsewhere:
- Overload ranking (S3O).
- Special members and record properties (S3C).
- Lambdas (WP-075). This spec gives only the generic-lambda hooks.
- Itanium mangling of specializations (WP-079).
- `linkonce_odr` emission (WP-080).

### 1.5 Deviations from clang 19

| # | clang 19 | kiln | Why |
|---|---|---|---|
| D1 | `runWithSufficientStackSpace` switches to a new stack, and warns "stack nearly exhausted" | the dedicated sema thread has a large stack; when `StackGuard` trips, kiln emits the fatal `kiln: stack exhausted during template instantiation; …` (or `parsing`, `constant evaluation`; §10.3) | safe Rust; no stack switching |
| D2 | `-ftime-trace` | also `-fkiln-instantiation-report[=json]` (§11.3) | accounting principle |
| D3 | always keeps `SubstTemplateTypeParmType` | the same by default; `-fno-subst-sugar` drops it (T3) | memory experiments |
| D4 | rebuilds every pattern subtree | can share non-dependent, non-local expression subtrees (§3.3, Q3) | memory; dumps stay byte-identical |
| D5 | no limit on subsumption | more than 65,536 DNF/CNF clauses is the error `constraints too complex to compare` (Q14) | bounded time |
| D6 | partial ordering reuses the argument template's own parameters as opaque types | synthesizes unique types at a reserved depth (§6.7) | standard wording; no depth aliasing |

None of these changes the accept/reject result of a program that clang 19
compiles within its own limits.

## 2. Parsing templates

The parser owns syntax only. Every name classification and node goes
through `Sema` actions (§2.8, §11.2). New code lives in
`kiln_parse/src/templ.rs`; `tokens.rs`, `decl.rs`, `expr.rs` and `stmt.rs`
get small hooks.

### 2.1 Template heads and parameters

```
template-head    := 'template' '<' template-parameter-list? '>' requires-clause?
template-param   := type-param | constrained-param | parameter-declaration | template-template-param
type-param       := ('class'|'typename') '...'? identifier? ('=' type-id)?
constrained-param:= type-constraint '...'? identifier? ('=' type-id)?
type-constraint  := nested-name-specifier? concept-name ('<' template-argument-list? '>')?
template-template-param := template-head ('class'|'typename') '...'? identifier? ('=' id-expression)?
```

**Kind decision.** These rules are clang's `isStartOfTemplateTypeParameter`.
1. `class` or `typename`, then any of `identifier`/`...`/`,`/`>`/`=`, then
   end, means a type parameter. This handles `typename T`,
   `class... Ts` and `typename = void`.
2. `typename` followed by a nested-name-specifier or a template-id
   (`typename T::type N`) means a non-type parameter.
3. `class`/`struct`/`union`/`enum` followed by an identifier and then
   `{`, `::` or `<` means a non-type parameter of class type.
4. An identifier or qualified name that `classify_name` reports as
   `Concept` (§2.8) means a constrained parameter. That includes
   `C<int> T` and `ns::C T`.
5. Otherwise the parser parses a `parameter-declaration` and gets a
   non-type parameter. Its type may be `auto`, `C auto` or
   `decltype(auto)`.

**Depth.**
- The parser keeps `template_depth: u16`. It is incremented for each
  enclosing **non-empty** template parameter list. `template<>` does not
  count, as in clang; golden `parse/depth-explicit-spec.cpp` checks this.
- Each parameter gets `(depth, index)`. An abbreviated `auto` parameter
  appends an invented parameter (§2.6).
- For out-of-line members such as `template<class T> template<class U>
  void A<T>::f(U)`, sema matches each list to a qualifier
  (`match_template_lists_to_scope`, clang's
  `MatchTemplateParametersToScopeSpecifier`, [temp.mem]/1). Errors use
  clang's texts:
  - "extraneous template parameter list in template specialization"
  - "template parameter list matching the non-templated nested type
    'X' should be empty"

**Defaults** are kept unparsed only where [temp.param]/15 requires
delayed meaning; in practice they are always parsed immediately.
- A default may name earlier parameters.
- Redefining a default is an error: "template parameter redefines default
  argument".
- Defaults are stored in the cold "template default args" map
  (ast.md §8.1).

### 2.2 Template-ids and the `<` rule

[temp.names]/3 says `<` starts a template-argument-list when it follows:
- a name whose lookup finds a template, or a template template parameter
  (`classify_name` returns `TypeTemplate`, `VarTemplate`,
  `FunctionTemplate` or `Concept`);
- an unqualified name whose lookup finds nothing or only functions, when
  the next token is `<`. This is P0846. It gives `UndeclaredTemplate`,
  and ADL resolves the name later.
- `operator` function-ids and literal-operator-ids that name templates;
- a name after the `template` keyword (§2.3);
- an injected-class-name used as a template-name ([temp.local]/1).

In every other case `<` is less-than.

**Closing the list.**
- The first top-level `>` closes the list. `(`, `[` and `{` nest and hide
  `>`.
- A `>>`, `>=`, `>>=` or `>>>` token that closes a list is split, as
  clang does ([temp.names]/4).
- `TokenSource::split_first(i)` records a *residual* token in the
  annotation side map (§2.8). `tok_at(i)` then yields the remainder, with
  kind `Punct` and `span.start + 1`. kiln_lex is unchanged.

**Each template argument.** This is [temp.arg.general]/2: an ambiguity
resolves to a type-id.
1. If the tentative scan `is_cxx_type_id(TypeIdContext::TemplateArg)`
   succeeds, the argument is a type. The scan is owned by WP-064. WP-069
   ships a stub: try `parse_type_name`, and rewind on failure.
2. If the argument is an id-expression that names a class or alias
   template, or a template template parameter, and the next token is
   `,`, `>` or `...`, it is a template-name argument.
3. Otherwise the parser parses a `conditional-expression` with
   `greater_is_operator = false`.

A trailing `...` wraps the argument in a pack expansion.

The parser builds `TemplateArg`s of kind Type, Expr, Template or
TemplateExpansion (types.md §8.3) and calls `act_template_id` (§11.2).

### 2.3 Disambiguators: `typename` and `template`

**`typename` is optional** in a *type-id-only context* ([temp.res.general]/4,
P0634). The parser passes `implicit_typename: bool` into
`parse_decl_specifiers` and `parse_type_id`. It is `true` for the terminal
name of:
- a typename-specifier, type-requirement, nested-name-specifier,
  elaborated-type-specifier, class-or-decltype or using-enum-declarator;
- the type-specifier of:
  - a new-type-id, defining-type-id, conversion-type-id or
    trailing-return-type;
  - the default argument of a type-parameter;
  - the type-id of a `static_cast`, `const_cast`, `reinterpret_cast` or
    `dynamic_cast`;
- a decl-specifier of:
  - a simple-declaration or function-definition **at namespace scope**;
  - a member-declaration;
  - a parameter-declaration in a member-declaration, in a function
    declaration whose declarator-id is qualified, in a lambda-declarator
    or in a requires-parameter-list (unless it is inside a default
    argument);
  - a parameter-declaration of a template-parameter.

At block scope `T::x * y;` is still an expression.

If `typename` is missing outside these contexts, clang's error and fix-it
apply: "missing 'typename' prior to dependent type name 'T::type'". The
parser then recovers as if `typename` had been written.

**`template`** is needed after `::`, `.` or `->` when the qualifier or
object is dependent and not the current instantiation, as in
`x.template f<int>()` and `T::template X<U>`. Without it, `<` is
less-than. When the name is followed by `<`, clang's diagnostic
`err_missing_dependent_template_keyword` ("use 'template' keyword to
treat 'f' as a dependent template name") is emitted and the parser
recovers as if `template` had been written.

### 2.4 Dependent nodes built at parse time

All of these are built by sema actions, which compute dependence as in
§3.2.

| Syntax (in a dependent context) | Node |
|---|---|
| `T::x`, or `X<T>::x` (not the current instantiation) in an expression | `DependentScopeDeclRefExpr {nns, name, targs?}` |
| `e.m` or `p->m` with a type-dependent object, or a member of the current instantiation that is not found when it has dependent bases | `CXXDependentScopeMemberExpr {base, arrow, nns?, name, targs?}` |
| unqualified `f(args)` with type-dependent args, where lookup finds functions or templates (or nothing) | `UnresolvedLookupExpr {requires_adl, overloaded, decls, targs?}`, plus `CallExpr` of dependent type |
| `obj.f(args)` that names an overload set of members | `UnresolvedMemberExpr` |
| `a op b` with a type-dependent operand | `CXXOperatorCallExpr` whose callee is an `UnresolvedLookupExpr` holding the operator functions found by unqualified lookup at definition ([temp.dep.candidate]); if none are found, a `BinaryOperator`/`UnaryOperator` of dependent type (clang) |
| `typename T::type` | `DependentName` type (canonical keyword `typename`, types.md §18) |
| `typename T::template X<U>` | `DependentTemplateSpecialization` type |
| `X<T>` where `X` is a class template | dependent `TemplateSpecialization` type |
| `T(args)` / `T{args}` with dependent `T` | `CXXUnresolvedConstructExpr` (list flag) |
| `int x(e)` with a type-dependent `e` | the initializer is kept as `ParenListExpr` |
| `sizeof...(P)` | `SizeOfPackExpr` |
| `e...` / `T...` | `PackExpansionExpr` / `PackExpansion` type |
| `(e op ...)` | `CXXFoldExpr` |
| `p...[i]` / `T...[i]` | `PackIndexingExpr` / `PackIndexing` type |
| `C<T>` in an expression | `ConceptSpecializationExpr` (+ `ImplicitConceptSpecialization` decl) |
| `requires (…) { … }` | `RequiresExpr` + `RequiresExprBody` decl |

### 2.5 Member templates, specialization and instantiation syntax

**Explicit specialization.**
- `template<> class-head` whose class-head-name is a template-id, as in
  `template<> struct A<int> { … }`.
- For members: `template<> void A<int>::f() {}`, and nested forms such as
  `template<> template<> void A<int>::g<char>()`.

**Partial specialization.** A non-empty template-head whose
class-head-name (or variable declarator-id) is a template-id:
`template<class T> struct A<T*>`, `template<class T> constexpr bool v<T*>`.

**Explicit instantiation.**
- A definition, `template` followed by a declaration with no `<`:
  `template struct A<int>;`, `template void f<int>(int);`,
  `template void f(int);` (the args are deduced),
  `template int A<int>::s;`.
- A declaration: `extern template …`.

**Friends.** `template<class U> friend struct B;`,
`friend void f<>(T);` (a friend specialization), and
`template<class U> friend void g(U);`.

**Deduction guide.** At namespace scope, a declarator whose name is a
class template visible in scope, followed by a balanced `( … )` and then
`->`. The parser detects it with a token scan, without tentative sema
actions. It parses `[explicit-specifier] template-name (params) ->
simple-template-id ;` and calls `act_deduction_guide`.

**Function template partial specialization** (`template<class T> void
f<T*>(T*)`) is parsed and then rejected by sema: "function template
partial specialization is not allowed".

### 2.6 Requires-clauses, requires-expressions and concepts

```
requires-clause      := 'requires' constraint-logical-or-expression
constraint-or        := constraint-and ('||' constraint-and)*
constraint-and       := primary-expression ('&&' primary-expression)*
requires-expression  := 'requires' ('(' parameter-declaration-clause ')')? '{' requirement+ '}'
requirement          := 'typename' nested-name-specifier? type-name ';'                 // type
                      | '{' expression '}' 'noexcept'? ('->' type-constraint)? ';'     // compound
                      | 'requires' constraint-expression ';'                           // nested
                      | expression ';'                                                 // simple
concept-definition   := 'template' template-head 'concept' identifier attribute* '=' constraint-expression ';'
```

**Requires-clauses.**
- One may follow a template-head, or follow a function declarator (a
  trailing clause, stored in the cold map, ast.md §8.1).
- Operands of `&&` and `||` must be primary expressions.
- For anything else clang's error applies ("parentheses are required
  around this expression in a requires clause", with a fix-it), and the
  parser recovers by parsing a full unary expression.

**Requires-expressions.**
- The parser picks the requirement kind from the leading token:
  `typename`, `{`, `requires`, or anything else.
- The parameters live in a `RequiresExprBody` DeclContext. Their scope is
  `ScopeKind::RequiresExpr`; this is a new kind.

**Concept definitions.**
- A concept cannot be constrained.
- The parser rejects a requires-clause after the template-head of a
  concept: "concept cannot have associated constraints".
- `concept bool` is rejected.

**Abbreviated function templates** ([dcl.fct]/22). Each parameter whose
type contains `auto` (`auto`, `C auto`, `auto&`, `auto...`) invents a
type parameter named `auto:N`, as in clang's dump.
- The invented parameter has depth `template_depth` (one more if the
  function had no template-head) and the next index.
- It goes into the function's template parameter list. The list is
  created on the first such parameter if it does not exist yet.
- `C auto` attaches the type-constraint `C<auto:N>`.
- Generic lambdas use the same routine (WP-075).

### 2.7 Pack expansions by context ([temp.variadic]/5)

| Context | Example | Parser site | M3 |
|---|---|---|---|
| function parameter pack | `void f(T... t)` | declarator | yes |
| template parameter pack that is a pack expansion | `template<T... v>` | §2.1 | yes |
| initializer-list / call arguments | `f(a...)`, `{a...}`, `T(a...)` | expression-list | yes |
| base-specifier-list | `struct D : B<T>... {}` | base clause (WP-065) | yes |
| mem-initializer-list | `D() : B<T>(0)... {}` | ctor-initializer (WP-065) | yes |
| template-argument-list | `X<T...>` | §2.2 | yes |
| using-declarator-list | `using B<T>::f...;` | using-decl (WP-062) | yes |
| alignment-specifier | `alignas(T...)` | attributes | yes |
| lambda capture | `[t...]`, `[...x = t]` | WP-075 | yes |
| `sizeof...` | `sizeof...(T)` | primary expression | yes |
| fold-expression | `(t + ...)` | parenthesized primary | yes |
| pack-index | `t...[0]`, `T...[0]` | postfix and type | yes |
| attribute-list | `[[a...]]` | — | rejected, as clang |
| structured-binding pack | `auto [...xs] = e` | — | M4 |
| friend pack | `friend T...;` | — | M4 |

**Fold expressions** ([expr.prim.fold]) take the 32 fold-operators.
After `(`:
1. If the next token is `...`, it is a left fold: `( ... op cast-expr )`.
2. Otherwise the parser parses a cast-expression. If a fold-operator then
   follows and is in turn followed by `...`, it is a right fold or a
   binary fold.

The operand must be a cast-expression. `(a + b + ...)` is an error:
"expression not permitted as operand of fold expression", with a fix-it
adding parentheses. If the two operators of a binary fold differ, the
error is "operators in fold expression must be the same".

### 2.8 Parser↔sema name-classification protocol

`classify_name` (sema.md, `decl.rs:2078`) gains a template-aware
signature:

```rust
pub fn classify_name(&mut self, name: Symbol, qual: Option<Qualifier>, next: NextTok) -> NameClass;
pub enum NextTok { Less, LParen, ColonColon, Other }              // 1 B
pub enum Qualifier { Decl(DeclId), Dependent(NnsId) }             // 8 B
pub enum NameClass {                                              // 12 B
    Type(QualType, DeclId), Namespace(DeclId), Value(DeclId), Undeclared,   // M2
    TypeTemplate(TemplateNameId),   // class/alias template, template template parm, BuiltinTemplate
    VarTemplate(DeclId),
    FunctionTemplate(DeclId),       // overload set containing ≥1 function template
    Concept(DeclId),
    UndeclaredTemplate,             // P0846: nothing (or only non-template fns) found and next == Less
    Dependent,                      // qualifier is dependent and not the current instantiation
}
```

**Protocol.**
1. When the parser sees an identifier (after any `::` chain parsed by
   `parse_nested_name`, which classifies each component the same way), it
   calls `classify_name`.
2. `TypeTemplate`, `VarTemplate`, `FunctionTemplate`, `Concept` or
   `UndeclaredTemplate`, followed by `<`, means a template-id (§2.2). The
   parser parses the args and calls `act_template_id(name, args, l, r)`,
   which returns:
   - `TemplateIdRef::Type(QualType)` for class and alias templates. The
     `TemplateSpecialization` type is formed and its specialization decl
     created, but nothing is instantiated.
   - `TemplateIdRef::Value(ExprId)` for variable templates and concepts.
   - `TemplateIdRef::Overload(…)` for function templates. It is kept until
     the call.
3. `Dependent` gives `DependentName` (in a type context with
   `typename`/implicit typename) or `DependentScopeDeclRefExpr`.
4. **Annotation.** A committed template-id or a resolved qualified type
   name is recorded as `Annot { start, end, kind: TemplateId(ref) |
   Type(QualType) | Scope(NnsId) }`. It lives in
   `TokenSource::annots: FxHashMap<u32 /*abs index*/, Annot>`.
   - When the parser reaches an annotated index again (after a rewind),
     it consumes the annotation and jumps to `end`. This mirrors clang's
     `annot_template_id`, `annot_typename` and `annot_cxxscope`.
   - Annotations pin the window: `release()` never frees past the oldest
     live annotation start or tentative pin.
   - Annotations are deleted once `pos` moves past their `end`.
5. **Tentative parsing** (`TentativeGuard`, owned by WP-064). While it is
   active:
   - sema accepts only *pure* queries: `classify_name`, lookup, and type
     interning;
   - it rejects actions that create decls, emit diagnostics or
     instantiate;
   - a template-id seen tentatively is skipped by bracket balancing after
     classification;
   - no annotation is created.
   Tentative failure rewinds `pos` and the bracket depth.

This protocol keeps kiln_lex unchanged and keeps token indices stable.
Classification is deterministic at a given point, so an annotation can
never go stale.

## 3. Dependent constructs and two-phase lookup

### 3.1 Dependent contexts and type dependence

**`Decl::DEPENDENT_CONTEXT`** is a flag bit set at decl creation. It is
true inside:
- a template pattern;
- a partial specialization;
- a member of either;
- a generic lambda's call operator.

It decides two things:
- whether an action builds a dependent node (§2.4) or checks fully;
- whether a nested class's `Record` type is dependent (clang's
  `isDependentContext`).

Type flags are computed at intern time (types.md §5.3). The rules:

| Type | DEPENDENT when | INSTANTIATION_DEPENDENT also when |
|---|---|---|
| `TemplateTypeParm` | always | — |
| `TemplateSpecialization` | the name is dependent, or any arg is dependent | any arg is instantiation-dependent |
| `DependentName`, `DependentTemplateSpecialization`, `InjectedClassName` | always | — |
| `Decltype(e)` | `e` is type-dependent, which gives `DependentDecltype` | `e` is instantiation-dependent |
| array, pointer, reference, function, member pointer | any component is dependent; an array bound is value-dependent | any component |
| `Record`/`Enum` | the decl is in a dependent context | — |
| `PackExpansion` | its pattern is dependent | — (and UNEXPANDED_PACK is cleared) |
| `Auto` | undeduced inside a dependent context | — |

### 3.2 Expression dependence (`ExprDeps`)

Dependence is computed once by each `AstBuilder` constructor, from its
operands' deps and its own type's flags (`deps_of_type_flags`). It is
never recomputed (ast.md §5). The rules follow [temp.dep.expr],
[temp.dep.constexpr] and clang's `ComputeDependence.cpp`.

| Expression | deps |
|---|---|
| any expression | the union of its children's deps, plus `deps_of_type_flags(type)` (gives TYPE when the type is dependent) |
| `DeclRefExpr` to an NTTP | VALUE \| INST (plus TYPE if the parameter's type is dependent; plus UNEXPANDED_PACK if it is a pack) |
| `DeclRefExpr` to a variable | TYPE if its type is dependent; VALUE if it is a const integral or `constexpr` variable whose initializer is value-dependent (ast.md §16.15) |
| `this` in a dependent context | TYPE |
| `sizeof`, `alignof`, `noexcept`, `typeid(type)`, `__is_*` traits, `requires` | the operand's TYPE becomes VALUE (the result type is fixed) |
| `noexcept(e)` with a can-throw result that is dependent | also VALUE \| INST |
| `ConceptSpecializationExpr`, `RequiresExpr` | VALUE if any argument is dependent |
| `SizeOfPackExpr` | VALUE \| INST (UNEXPANDED_PACK is cleared) |
| `PackExpansionExpr` | its pattern's deps without UNEXPANDED_PACK |
| `CXXFoldExpr` | its operands' deps without UNEXPANDED_PACK (for the pack operand) |
| `UnresolvedLookupExpr` | TYPE \| VALUE \| INST if any explicit template arg is dependent; otherwise `requires_adl` decides at the `CallExpr` |
| a lambda in a dependent context | INST |
| any of the above with an error operand | plus ERROR |

INST alone means the expression mentions a template parameter but neither
its type nor its value depends on it, as in `sizeof(sizeof(T))`. Such an
expression must still be substituted (it is not shared, §3.3) and it
counts in [temp.over.link] equivalence.

### 3.3 Checks at definition time (clang's behaviour)

The general rule:
- When an action receives a dependent operand, it skips the semantic
  check and builds the dependent node of §2.4.
- A non-dependent operand or subtree is fully checked **once**, at
  definition. This covers the diagnostics, its types and the implicit
  conversions.

Diagnosed at definition, as clang does:
- An unqualified non-dependent name not found: "use of undeclared
  identifier 'x'". This includes members of dependent bases, which are
  not searched ([temp.dep.general]/4). The exception is an unqualified
  call with a type-dependent argument, where ADL may still find it.
- A non-dependent expression that is ill-formed (`int x = "a";`), or a
  non-dependent call with no viable candidate.
- A member of the current instantiation that is not found when the class
  has no dependent bases: "no member named 'x' in 'A<T>'".
- A missing `typename` or `template` (§2.3), a redefined default
  argument, a malformed template head.

Not diagnosed at definition:
- `static_assert(false)`, or any non-dependent `static_assert` failure
  inside a template (P2593, clang 17+).
- "No valid specialization" ([temp.res.general]/6). This is ill-formed
  NDR; clang does not diagnose it and neither does kiln.

**Binding.** A non-dependent name is bound at definition. Its
`DeclRefExpr` stores the decl that was found, so declarations added later
are invisible to it ([temp.res.general]/1).

**Sharing (optimization O1, Q3).**
- During substitution, an expression subtree is reused **by id** when
  `deps & (TYPE|VALUE|INST|UNEXPANDED_PACK|LOCAL) == 0`.
- `ExprDeps::LOCAL = 32` is a new bit. A builder sets it on a
  `DeclRefExpr` to a parameter, local variable, local class or lambda
  that belongs to a dependent context, and the bit propagates upward like
  the others.
- Statements and decls are never shared.
- The AST then becomes a DAG. Dumps are unchanged, because the dumper
  prints a shared subtree at each use.
- `-fno-share-nondependent` turns sharing off for differential testing.
- The expected saving is 30–60% of the expression nodes in libc++ bodies.
  It is measured by the instantiation report (§11.3).

### 3.4 Current instantiation ([temp.dep.type]/1)

These are the current instantiation:
- inside a class template (or a partial specialization), the
  injected-class-name;
- `A<T>` or `A<T...>` written with the class's own parameters in order;
- the nested classes of either.

Qualified lookup into the current instantiation happens at definition.
Lookup of a found member binds it (`typename A<T>::X` resolves to the
member typedef, not to `DependentName`). If the member is not found and
the class has dependent bases, the result is a dependent node.

### 3.5 ADL and operator lookup at instantiation ([temp.dep.candidate])

When a dependent unqualified call or operator is substituted:
1. The candidates are the decls stored in the `UnresolvedLookupExpr`
   (found by unqualified lookup at definition).
2. Then ADL runs on the *instantiated* argument types, against the
   **current** state of the namespace tables.
3. For operators, the built-in candidates are added (S3O).

Kiln, like clang, does not model the lookup scope at each point of
instantiation. Namespace contents are append-only, so ADL at instantiation
sees every declaration made up to that moment. Pending instantiations run
at end of TU, so they see the whole TU. This matches clang's results in
every case clang accepts. (The difference is ill-formed NDR,
[temp.point]/7.)

## 4. Class templates

### 4.1 Tables (`kiln_ast::templates`)

```rust
pub struct TemplateTables {
    infos: Vec<TemplateInfo>,                 // by TemplateIndex (TemplLink::Pattern payload)
    specs: Vec<SpecInfo>,                     // by SpecId, 24 B (ast.md §8.3)
    member_specs: Vec<MemberSpecInfo>,        // by MemberSpecId, 12 B
    specializations: HashMap<(DeclId, ArgListId), DeclId>,   // canonical args; hashbrown (T1)
    partials: HashMap<DeclId, ListId>,        // template -> partial specs, in declaration order
    instantiated_from: HashMap<DeclId, DeclId>,  // sparse: Field, EnumConstant, ParmVar, Typedef, UsingShadow, locals
    member_map: HashMap<(DeclId /*spec*/, DeclId /*pattern member*/), DeclId>,
    param_objects: HashMap<(QualType, ConstValueId), DeclId>,
}
pub struct TemplateInfo {                    // 24 B
    params: TemplateParamListId, pattern: DeclId,
    member_of: Option<DeclId>,               // member template of a class specialization: the pattern member template
    partial_of: Option<DeclId>,              // for partial specializations
    first_spec: Option<DeclId>, flags: TemplFlags /*u16: HAS_PARTIALS, IS_ALIAS, BUILTIN, …*/,
}
```

**Which link each decl gets** (ast.md §8.2):

| Decl | Link |
|---|---|
| pattern of a class, function or variable template, or of a partial specialization | `Pattern(TemplateIndex)` |
| class, function or variable specialization (implicit or explicit) | `Specialization(SpecId)` |
| member function, static data member, member class or member enum of a class specialization | `MemberOfSpecialization(MemberSpecId)` |
| friend template whose target is dependent | `DependentFriend` |
| any other decl that was instantiated | `instantiated_from` (sparse map) |

`member_map` serves `find_instantiated_decl` (§5.3) for members of class
specializations. Function-local decls use the scoped `LocalInstScope` of
the running `Substituter` instead.

### 4.2 Forming a specialization: `get_class_spec(tmpl, written, loc)`

1. `check_template_args(tmpl, written)` ([temp.arg]):
   - matches kinds and arity;
   - fills missing args from defaults, substituting the earlier args into
     them (frame `DefaultTemplateArgumentInstantiation`);
   - converts NTTP args (converted constant expressions, evaluated by the
     Oven with `EvalKind::TemplateArg`);
   - packs the trailing args into a `Pack`.
   It returns `ConvertedArgs { sugared: ArgListId, canonical: ArgListId }`.
2. Probe `specializations[(first(tmpl), canonical)]`. On a hit, return
   that decl.
3. On a miss, create a `ClassTemplateSpecialization` decl with
   `SpecInfo{kind: Undeclared, args: sugared}` and insert it.
   - No instantiation happens. The type `X<int>` is
     `TemplateSpecialization(sugar) → Record(spec)`.
   - This is clang's `CheckTemplateIdType`.

With dependent args, steps 2–3 are skipped. The result is a dependent
`TemplateSpecialization` type, unless it names the current instantiation
(§3.4).

### 4.3 Triggers for implicit instantiation ([temp.inst]/2)

Kiln instantiates through exactly one hook,
`Sema::require_complete(ty, loc, diag) -> bool`, which is used wherever
sema.md and S3C require a complete type. Those places are:
- defining an object;
- member access and qualified lookup into `X<…>::`;
- `sizeof`/`alignof`;
- conversion to a base, and derived-to-base checks during overload
  resolution;
- `new`, `delete`, `throw`, `catch` by value;
- a by-value parameter or return at a definition or call;
- pointer arithmetic;
- a base-specifier;
- the traits marked "complete" in §9.

`is_complete(ty)` (no instantiation) exists for the places where the
standard says "if the class is complete". Clang calls `isCompleteType`
there, which also instantiates, so kiln uses `require_complete`
silently (`diag = None`) at those sites. S3O lists them.

### 4.4 Selecting the pattern ([temp.spec.partial.match], [temp.spec.partial.order])

1. Let `P = partials[tmpl]` (in declaration order) and `A` the canonical
   args.
2. **Prefilter.** Each partial carries an `ArgShape` per argument:
   - the canonical `TypeClass` of a type whose top level is non-dependent;
   - the value of an NTTP that is not dependent;
   - otherwise `Any`.
   A partial whose shape mismatches `A` is skipped without deduction. The
   shapes are computed when the partial is declared.
3. For each partial `p` that remains:
   - deduce `p.params` from `(p.args, A)` with `deduce_args` (§6.2);
   - finish the deduction: substitute the deduced args into `p.args` and
     require canonical equality with `A`;
   - check `p`'s associated constraints (§8).
   The partials that succeed form the set `M`.
4. If `M` is empty, the pattern is the primary template. If `M` has one
   element, it is that partial. Otherwise the pattern is the most
   specialized partial, found by function-template partial ordering of
   the rewritten `template<p.params> void f(X<p.args>)` (§6.7), with
   constraints as the tie-breaker.
5. If no unique best partial exists, the error is "ambiguous partial
   specializations of 'X<int *>'", with a note "partial specialization
   matches [with T = int]" for each candidate.
6. Record the choice in `SpecInfo.from` and `SpecInfo.from_args` (the
   deduced args of the partial).

Explicit specializations never reach this step: they are found by the
probe in §4.2.

### 4.5 Explicit specializations and instantiations

**Placement** ([temp.expl.spec]).
- An explicit specialization is allowed in any scope where the primary
  template may be defined. That includes class scope (CWG727).
- It must come before the first use that would cause implicit
  instantiation. Otherwise the error is "explicit specialization of
  'X<int>' after instantiation", with the note "implicit instantiation
  first required here" pointing to `SpecInfo.point_of_instantiation`.

**Allowed `SpecKind` transitions** (clang's `CheckSpecializationInstantiationRedecl`):

| from \ to | ExplicitSpecialization | ExplInstDecl | ExplInstDef |
|---|---|---|---|
| Undeclared | ok | ok | ok (instantiate everything) |
| ImplicitInstantiation | error: after instantiation | ok | ok |
| ExplicitSpecialization | redeclaration ok | ignored | ignored (no diagnostic, as clang) |
| ExplInstDecl | error | ok | ok |
| ExplInstDef | error | ignored | error: "duplicate explicit instantiation of 'X<int>'" |

**An explicit instantiation definition** of a class ([temp.explicit]/10)
instantiates the definitions of every member that is defined in the
pattern and not yet explicitly specialized. Member templates are
excluded.

**An explicit instantiation declaration** (`extern template`) does not
stop sema from instantiating inline and `constexpr` members when they
are used. It only tells codegen not to emit them (WP-080).

**Member specializations** such as `template<> void A<int>::f()`:
1. Instantiate the declaration of `A<int>` (declarations only).
2. Find the member `f` through `member_map`.
3. Set its `MemberSpecInfo.kind = ExplicitSpecialization` and attach the
   new definition.

### 4.6 `instantiate_class(spec, poi)` and member-wise laziness

1. If the specialization already has a definition, or is invalid, return.
2. Choose the pattern (§4.4).
   - If the pattern has no definition: "implicit instantiation of
     undefined template 'X<int>'", with the note "template is declared
     here". The class stays incomplete and is not marked invalid (clang).
3. Push the frame `TemplateInstantiation(Class)`; this checks depth and
   the stack guard. Then `Ast::begin_instantiation()`.
4. Build `MultiLevelArgs` (§5.1). The innermost level holds the spec's
   args, or the partial's deduced args. The outer levels come from the
   enclosing specializations.
5. Instantiate, in pattern order:
   - **base-specifiers**, expanding packs. A base must be complete, which
     can recurse.
   - **attributes** that affect layout (`alignas`, `packed`,
     `trivial_abi`).
   - **member declarations**. [temp.inst]/3 decides what happens to each
     kind of member:

| Member | At class instantiation | Definition instantiated when |
|---|---|---|
| non-static data member | declaration with the substituted type (it must be complete) | — |
| NSDMI | not instantiated | a constructor or aggregate initialization uses it (§4.9) |
| static data member | declaration (type only) | it is odr-used, or its value is needed for constant evaluation (§4.11) |
| member function | declaration: signature; exception spec `Deferred`; default args deferred | it is odr-used (§6.5); for `constexpr`, `consteval` and deduced-return functions, immediately at the reference |
| member class | declaration (a `Record` with `MemberOfSpecialization`) | it must be complete |
| unscoped member enum | **definition** | — |
| scoped member enum | declaration | it must be complete, or an enumerator is named |
| typedef, alias declaration, `using` declaration | fully | — |
| `static_assert` | fully: substituted and evaluated now | — |
| member template | declaration: a new template decl, with outer levels substituted and its own parameters retained (§5.1); `TemplateInfo.member_of` points to the pattern member | its own specializations need it |
| friend | declaration; a hidden friend's definition is in `instantiated_from` | odr-use (§4.9) |
| anonymous union | definition | — |

6. Complete the class:
   - implicit special-member declarations (S3C);
   - the layout, which stays lazy until abi asks for it.
7. Record `point_of_instantiation = poi` and set `kind =
   ImplicitInstantiation`.
8. `Ast::end_instantiation()`, record the report entry (§11.3), and pop
   the frame.

Any error during instantiation marks the specialization `invalid` (P7).
Later uses of it are silent.

**Points of instantiation.**
- `SpecInfo.point_of_instantiation` is the location of the use that first
  required the definition, as clang records it. It is used only in
  "requested here" notes and in the explicit-specialization-after-
  instantiation check.
- Function and variable definitions that are not needed immediately go on
  the `pending` worklist with their POI. `end_of_tu_instantiations()`
  drains the worklist with a loop; an instantiation may push more entries
  onto it.

### 4.7 Injected-class-name

- Inside the pattern, `A` names an `InjectedClassName` type. Its
  canonical type is the dependent `A<T…>` (types.md T6).
- `A` followed by `<`, or used as a template template argument, is the
  template-name ([temp.local]/1–2).
- Substituting an `InjectedClassName` gives
  `Record(get_class_spec(tmpl, level args))`. It always hits the map,
  because the specialization exists.
- Inside a partial specialization, the injected-class-name is the
  partial's own pattern args, as written.

### 4.8 Friends

- A friend function **defined** in a class template (a hidden friend) is
  instantiated as a declaration into the innermost enclosing namespace,
  visible only to ADL. Its body is instantiated on odr-use.
- If its signature does not depend on the template's parameters, the
  second specialization to instantiate it reports "redefinition of 'f'"
  (clang). The note shows the first instantiation.
- A friend template, or a friend specialization (`friend void f<>(T)`),
  is substituted and then matched against the namespace-scope templates
  when it is instantiated.
- A friend class (`friend class B<T>;`) is substituted and grants access
  (S3C).

### 4.9 Default member initializers

- The NSDMI of a member of a specialization is instantiated the first
  time a constructor that uses it is defined, or an aggregate
  initialization reads it. It is cached in the field's cold slot.
- A recursive use while the enclosing class is being instantiated is
  clang's error: "default member initializer for 'x' needed within
  definition of enclosing class 'A<int>' outside of member functions".
- The frame is `DefaultMemberInitializerInstantiation`.

### 4.10 Alias templates

- An alias template `A<args>` never becomes a decl. Its aliased type is
  substituted **at type formation**, even when the args are dependent
  (the result is then dependent).
- The result is `TemplateSpecialization{name: A, args: as written,
  aliased: result}`, which is sugar (types.md §5.3). The canonical type
  is the canonical aliased type.
- `alias_cache: (alias, canonical args) → canonical result` makes repeated
  forms cost one probe.
- Alias substitution is in the immediate context: a failure inside a
  deduction is SFINAE, including for args the alias does not use
  (CWG1558, the `void_t` idiom).
- A pack expansion as the argument for a non-pack parameter is
  diagnosed when the alias is dependent: "pack expansion used as argument
  for non-pack parameter of alias template" (clang, CWG1430).

### 4.11 Variable templates and static data members (and the Oven)

**Variable templates.** `v<args>` calls `get_var_spec`, which is §4.2
with a `VarTemplateSpecialization` decl.
- The type is substituted immediately: the expression needs it.
- Partial specializations are selected as in §4.4.
- The `inline constexpr bool xxx_v` family (142 declarations in the M3
  headers, §13.3) is the hottest path. One probe gives the decl and one
  `consts()` lookup gives its value.

**Initializers.** Static data members and variable specializations follow
the same rule, which is clang's `DoMarkVarDeclReferenced`.
- A variable that is *usable in constant expressions* (`constexpr`, or
  const integral/enum with an initializer, or a reference) has its
  initializer instantiated **immediately** when it is referenced. Its
  value is then evaluated and cached in `consts()`.
- Other variables are queued in `pending` when they are odr-used.

This is why the chain `R<N>::v = R<N-1>::v` recurses one frame per level
in clang (the d1 test in §10.2). Kiln matches its notes exactly.

**The Oven never sees an uninstantiated variable.** By the time an
initializer is evaluated, each variable it references has already been
instantiated and evaluated, because building the reference marked it.

**Functions differ.** An evaluation can reach a function definition that
was never marked, through a function pointer or a virtual call. For
those, `SemaBridge::ensure_body(f, oven)` (sema.md §8.1; as landed by
WP-070b):
1. If `f` already has a parsed or instantiated definition, its body is
   returned.
2. Otherwise, if `f` is a specialization or a member of a class
   specialization (`instantiated_from(f)` is set), the evaluation is
   manifestly constant-evaluated (`eval_const`, which records its
   location in `InstState.eval_loc`), and `f` was **never marked**
   (it is not in the pending queue), `ensure_body` instantiates it now.
   The frame is `TemplateInstantiation(Function)` and the POI is the
   evaluation's location. It returns the body.
3. Otherwise it returns `None`, and the Oven reports its "undefined
   function cannot be used in a constant expression" note. This
   covers:
   - a speculative fold (`try_fold`): it never instantiates;
   - a failed instantiation, or one after a fatal error;
   - a member that was marked but whose instantiation is still
     deferred because its pattern was not yet defined at the mark
     (`InstState.queued`). clang also leaves it undefined: the test
     `templates/diag/temp.inst-constexpr-defined-later.cpp` pins the
     note.

**Requirement R-OVEN-1 (reentrancy)**, as landed:
- `Sema.oven` is an `Option<Oven>`. `with_oven` takes it out for the
  duration of an evaluation and hands the closure a `Bridge` that holds
  `&mut Sema`.
- In `ensure_body`, the Oven passes its own `&mut Oven`. Sema lends it
  back by swapping it into `Sema.oven` (a fresh `Oven` with the same
  budget holds its place) while it instantiates. Nested evaluations
  (`static_assert`, template arguments, `if constexpr`, constexpr
  variable initializers) therefore run on the same Oven, sharing its
  compiled code, memo cache and budget. The Oven is swapped back before
  `ensure_body` returns.
- `crates/kiln_parse/tests/ensure_body.rs` pins the behaviour. Its
  calls are built with the AST builder so they bypass marking. The
  tests cover:
  - an unmarked member instantiated once (the second evaluation finds
    the body) and recorded in the report;
  - a speculative fold that does not instantiate;
  - a nested `static_assert` in the instantiated body;
  - an instantiation error that is reported with its context, while
    the definition still exists, as in clang;
  - a member marked before its pattern was defined, which is
    instantiated at the end of the TU.
- No fallback is needed (pre-instantiating everything odr-used).

## 5. Substitution: the tree transform

### 5.1 Multi-level arguments

```rust
pub struct MultiLevelArgs {                    // clang MultiLevelTemplateArgumentList
    levels: SmallVec<[Level; 4]>,              // index = template depth, outermost first
}
pub struct Level { args: ArgListId /*converted, sugared*/, assoc: DeclId, mode: LevelMode }
pub enum LevelMode { Substitute, Retain }      // Retain: the level's own params stay parameters
```

`template_args_for(decl)` walks a decl's DeclContexts outward. At each
enclosing specialization it pushes that specialization's args (or the
deduced partial args), and gives the result.

A parameter `(d, i)`:
- if level `d` is `Substitute`, maps to `args[i]`;
- otherwise it is rebuilt as a parameter at depth `d - k`, where `k` is
  the number of `Substitute` levels below `d`. The rebuilt parameter
  refers to the new param decl, found through `LocalInstScope`.

This is how the member template `A<int>::f<U>` ends up with `U` at depth
0, as in clang.

### 5.2 The `Substituter`

```rust
pub struct Substituter<'s, 'a, 'sm> {
    sema: &'s mut Sema<'a, 'sm>,
    args: MultiLevelArgs,
    locals: LocalInstScope,   // scoped FxHashMap<DeclId, DeclOrPack>: pattern local -> instantiated
    pack_index: Option<u32>,  // current element during an expansion (clang ArgumentPackSubstitutionIndex)
    entity: DeclId,           // what is being instantiated; owner for created decls
}
pub enum DeclOrPack { Decl(DeclId), Pack(ListId) }   // a function parameter pack -> N ParmVars
pub struct SubstFail;                                // a SFINAE failure; the diagnostic is in the trap
impl Substituter<'_, '_, '_> {
    pub fn ty(&mut self, t: QualType, loc: Loc) -> Result<QualType, SubstFail>;
    pub fn expr(&mut self, e: ExprId) -> Result<ExprId, SubstFail>;
    pub fn stmt(&mut self, s: StmtId) -> Result<StmtId, SubstFail>;
    pub fn decl(&mut self, d: DeclId, owner: DeclId) -> Result<DeclId, SubstFail>;
    pub fn args(&mut self, l: ArgListId) -> Result<ArgListId, SubstFail>;  // expands packs
    pub fn nns(&mut self, n: NnsId, loc: Loc) -> Result<NnsId, SubstFail>;
}
```

Outside SFINAE mode, a failure has already been diagnosed. The caller
then gets an ERROR-flagged node and goes on, which keeps error recovery
like clang's.

### 5.3 Types

1. If `t` has none of `DEPENDENT`, `INSTANTIATION_DEPENDENT` or
   `UNEXPANDED_PACK`, return `t`. Types are interned, so this reuse costs
   nothing.
2. Otherwise substitute according to the kind:

| Kind | Rule |
|---|---|
| `TemplateTypeParm(d,i)` | look up the arg. A pack param uses `pack_index` to pick an element; with no index inside an expansion it gives `SubstTemplateTypeParmPack`. Wrap the result in `SubstTemplateTypeParm{replacement, assoc, index, pack_index}`. Cv-qualifiers on the parameter are merged into the replacement; cv on a reference is dropped. |
| pointer, reference, array, function, member pointer | rebuild through the *checked* `TypeContext` constructors (§5.7). These fail on a reference to `void`, a pointer to a reference, an array of functions, an array bound ≤ 0, a function returning an array or function, or a member pointer into a non-class. |
| `TemplateSpecialization` | substitute the name and args. An alias goes to §4.10. A class goes to `get_class_spec` and does **not** instantiate. |
| `DependentName{nns, id}` | substitute `nns`. If it is a class: `require_complete`, then qualified lookup for a type. The failures are clang's: "no type named 'x' in 'Y'", "typename specifier refers to non-type member 'x' in 'Y'", "type 'int' cannot be used prior to '::' because it has no members". |
| `DependentTemplateSpecialization` | the same, looking for a template and then forming the specialization |
| `DependentDecltype(e)` | substitute `e` in an unevaluated context, then apply the [dcl.type.decltype] rules |
| `UnaryTransform` | substitute the operand, then apply the builtin (§9.2) |
| `PackExpansion`, `PackIndexing` | §7 |
| `InjectedClassName` | §4.7 |
| `Record`/`Enum`/`Typedef` of a decl in a dependent context | `find_instantiated_decl(d)`: a local goes through `locals`; a member goes through `member_map` of the enclosing specialization, and the member class is instantiated if completeness is needed |
| `Auto` | a deduced `Auto` substitutes its deduced type; an undeduced one stays and is deduced in the instantiation |

### 5.4 Expressions

For each kind, the transform substitutes the children and then calls the
**same `Sema` action the parser calls**, e.g. `act_binary`, `act_call`
and `act_member`. Every check and implicit conversion is therefore
recomputed exactly as for non-template code (clang's
`TreeTransform::Rebuild*`).

- **Implicit nodes are stripped, then rebuilt by the action.** This
  covers `ImplicitCastExpr`, `MaterializeTemporaryExpr`,
  `CXXBindTemporaryExpr`, `ExprWithCleanups` and the `ConstantExpr`
  wrappers. `ParenExpr` is kept.
- **Sharing.** A non-dependent subtree is shared (§3.3), unless sharing is
  turned off.
- **Dependent forms** are resolved:
  - `DependentScopeDeclRefExpr` becomes qualified lookup;
  - `UnresolvedLookupExpr` becomes an overload set plus ADL (§3.5);
  - `CXXDependentScopeMemberExpr` becomes member lookup;
  - `CXXUnresolvedConstructExpr` becomes [expr.type.conv];
  - `ParenListExpr` becomes the initialization of its variable.
- **An NTTP reference** becomes `SubstNonTypeTemplateParmExpr {param,
  pack_index, replacement}`. The replacement is:
  - an `IntegerLiteral` (or the matching literal) built from the
    `ConstValueId`;
  - for a class-type NTTP, a `DeclRefExpr` to the `TemplateParamObject`
    (§6.10);
  - for a reference or pointer NTTP, the entity expression.
  This matches clang's dump.
- **`ConceptSpecializationExpr`, `RequiresExpr`.** §8.
- **Lambdas** get a new closure class per instantiation (WP-075).
- **`SizeOfPackExpr`, folds, expansions.** §7.

### 5.5 Statements

- Statements are substituted recursively. A `DeclStmt` goes through
  `decl()` and binds `locals`.
- **`if constexpr`.** Substitute the condition, then evaluate it with
  `EvalKind::IfConstexpr`. Only the taken branch is instantiated; the
  discarded slot stays empty (ast.md §5, [stmt.if]/2).
- **Range-`for`.** Re-run the desugaring of [stmt.ranged] on the
  substituted range expression. This builds new `__range`, `__begin` and
  `__end` variables.
- **`return` in a deduced-return function** deduces from the first
  non-discarded `return` (§6.3).

### 5.6 Declarations

- **Order.** The function signature is substituted in lexical order
  ([temp.deduct.general]/7): a leading return type, then the parameters,
  then a trailing return type, then the trailing requires-clause (§8.3).
  The exception spec and `explicit(bool)` come later, when needed
  ([temp.inst]/14).
- **Parameters.** `ParmVar`s are substituted. A function parameter pack
  becomes N `ParmVar`s (`DeclOrPack::Pack`).
- **Local classes** inside an instantiated body are instantiated fully
  and immediately ([temp.inst]/2).
- **Member templates** are created as in §4.6.
- **`static_assert`** is evaluated with the substituted condition.
- **`using` declarations** with packs give a `UsingPack` decl.

### 5.7 SFINAE

```rust
pub enum DiagMode { Report, Sfinae(Box<SfinaeState>) }
pub struct SfinaeState { first: Option<Diagnostic>, errors: u32 }
impl Sema<'_, '_> {
    pub fn with_sfinae<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> (R, Option<Diagnostic>);
}
```

**What SFINAE mode does** (clang `SFINAETrap` +
`Sema::EmitCurrentDiagnostic`):
- An *error* is captured, not emitted; the first one is kept. It also
  makes the current substitution return `SubstFail`.
- Warnings and remarks are stored in
  `suppressed: FxHashMap<DeclId, Vec<Diagnostic>>`, keyed by the
  specialization being formed. They are emitted when that specialization
  is first used (`diagnose_use_of_decl`). This is clang's
  `SuppressedDiagnostics`.
- Fatal errors are always reported: the depth limit and stack
  exhaustion.

**When SFINAE mode applies** is decided by the instantiation stack, not
by the caller. Each `InstFrame` has `sfinae: Option<bool>` (§10.1):
- `DeducedTemplateArgumentSubstitution`,
  `ExplicitTemplateArgumentSubstitution`, `ConstraintSubstitution`,
  `RequirementInstantiation` and
  `NestedRequirementConstraintsCheck` are `Some(true)`;
- `TemplateInstantiation`, `DefaultFunctionArgumentInstantiation`,
  `DefaultMemberInitializerInstantiation` and
  `ExceptionSpecInstantiation` are `Some(false)`;
- every other frame is `None`, meaning it inherits.

`is_sfinae()` is the innermost `Some`. It is computed when a frame is
pushed and stored, so it costs O(1).

This gives [temp.deduct.general]/8 directly:
- substitution into the function type, the template parameter types and
  alias templates happens directly under a `Some(true)` frame, so it is
  in the *immediate context*;
- any class, function or variable *definition* instantiated meanwhile
  pushes a `Some(false)` frame. Its errors are therefore hard errors,
  printed with the whole stack, and that specialization is marked
  invalid.

**Failure info** is attached to a non-viable candidate (§10.4):

```rust
pub struct DeductionFailure {        // ≤ 40 B
    kind: Tdk, param: Option<u16>, first: TemplateArg, second: TemplateArg,
    call_arg: Option<u16>, diag: Option<Box<Diagnostic>>, sat: Option<SatId>,
}
pub enum Tdk { Invalid, InstantiationDepth, Incomplete, IncompletePack, Inconsistent, Underqualified,
    SubstitutionFailure, DeducedMismatch, DeducedMismatchNested, NonDeducedMismatch,
    TooManyArguments, TooFewArguments, InvalidExplicitArguments,
    NonDependentConversionFailure, ConstraintsNotSatisfied, MiscellaneousDeductionFailure, AlreadyDiagnosed }
```

`Tdk` mirrors clang's `TemplateDeductionResult`.

**Memory.** Types and nodes that a failed substitution created are
retained, since the AST is append-only. They are metered as
`templ.sfinae_discarded` (§11.3).

## 6. Function templates

### 6.1 Pipeline for a call: `deduce_call(ft, explicit, args, ctx)`

1. **Explicit args** ([temp.arg.explicit]).
   - Check them against the leading parameters and convert them.
   - Substitute them into the function type under the
     `ExplicitTemplateArgumentSubstitution` frame. Parameters without
     explicit args stay parameters.
   - A trailing pack that was given explicitly can be *extended* by
     deduction (/9).
   - Failure gives `InvalidExplicitArguments` or `SubstitutionFailure`.
2. **Deduce from each (Pᵢ, Aᵢ) pair** ([temp.deduct.call]). Pᵢ is the
   parameter type; Aᵢ is the argument's type and value category.
   - **Adjustments, when P is not a reference.** A undergoes
     array-to-pointer and function-to-pointer decay, and its top-level cv
     is dropped. Top-level cv on P is dropped.
   - **When P is a reference.** Use the referred type. A forwarding
     reference (`T&&`, where `T` is this template's own unqualified
     parameter) with an lvalue A uses A&.
   - **A braced list.** If P is `initializer_list<P'>` or `P'[N]`, deduce
     P' from each element (and N). Otherwise it is non-deduced.
   - **An overload set.** If exactly one member deduces, use it (/6). If
     the set contains a template, it is non-deduced.
   - **A trailing function parameter pack.** Match every remaining A
     against the pattern. A pack that is not last is non-deduced.
   - **A P without template params.** It is not deduced. Its conversion
     is checked after deduction (CWG1391) and gives
     `NonDependentConversionFailure`.
3. **Finish** ([temp.deduct.general]/5):
   - an undeduced parameter takes its default (substituted in order); a
     pack takes the empty pack; anything else gives `Incomplete`;
   - substitute everything into the function type under
     `DeducedTemplateArgumentSubstitution`;
   - check that the deduced A is compatible ([temp.deduct.call]/4:
     equal, more cv-qualified, qualification-convertible, or a derived
     class). Failure is `DeducedMismatch`.
   - check the associated constraints (§8). The order is clang 19's,
     *after* the function type substitution (Q12, CWG2369).
4. **Success.** `get_function_spec(ft, canonical)` memoizes and returns
   the `FunctionTemplateSpecialization` decl. It has the substituted
   type and is a declaration only.

S3O ranks the result. The **body** of the winning candidate is
instantiated by `mark_referenced` when the call is built (§6.5).

### 6.2 The deduction core: `deduce(P, A, flags, &mut DeducedArgs) -> Tdk`

This is [temp.deduct.type]. It is a structural recursion over canonical
`P` against canonical `A`, guarded by the stack guard.

`flags` reproduce clang's `TDF_*`:
- `ParamWithReferenceType`, `IgnoreQualifiers`, `DerivedClass`;
- `SkipNonDependent`, `TopLevelParameterTypeList`;
- `InOverloadResolution`, `ArgWithReferenceType`;
- `AllowCompatibleFunctionType`.

**Deduced forms:**
- `T`, cv `T`, `T*`, `T&`, `T&&`, `T[i]`;
- `T(params) noexcept(b)`, `T C::*`;
- `TT<args>` and `class-template-id<args>`;
- an NTTP value `i` from an array bound or a template argument.

A class template-id P with a class A tries A first. If that fails and
`DerivedClass` is set, it tries each base of A
([temp.deduct.call]/4.3). The success must be unique; otherwise the
result is `MiscellaneousDeductionFailure` (clang).

**Non-deduced contexts:**
- the nested-name-specifier of a qualified-id;
- `decltype(e)`;
- an NTTP inside a subexpression (`N+1`);
- a parameter with a default argument that the call does not use;
- an overload set that contains a template;
- a braced list (except as in §6.1);
- a pack expansion that is not last;
- a pack-index (P2662).

**Consistency.** Two deductions of the same param must be canonically
equal. For NTTPs, the values must be equal after conversion to the
parameter's type ([temp.deduct.type]/17). A mismatch gives
`Inconsistent` with both values.

`deduce_args(Ps, As)` applies this to template argument lists, with pack
handling as in §7.2. It is shared by partial-spec matching (§4.4),
partial ordering and template template argument matching (P0522: "at
least as specialized as").

### 6.3 `auto` and `decltype(auto)` through the same machinery

`deduce_auto(declared, init, kind) -> Result<QualType, AutoFail>`
([dcl.type.auto.deduct]):
1. Replace `auto` with an invented param `U`. It sits at the reserved
   depth `AUTO_DEPTH = 0x7FFD` (no aliasing with real params).
2. Run §6.1 step 2 as if for `f(init)` with P = the declared type.
   - Copy-list `auto x = {…}` uses `initializer_list<U>`.
   - Direct-list `auto x{e}` needs exactly one element (N3922).
3. `decltype(auto)` does not deduce. It applies the [dcl.type.decltype]
   rules to `init`. Anything other than plain `decltype(auto)` is clang's
   error.
4. `C auto`: check `C<deduced, args…>` (§8). On failure, "deduced type
   'int' does not satisfy 'C'" plus the note chain.

**Uses:**
- variables;
- `new auto(…)`;
- NTTPs (`template<auto V>`, `C auto V`, `decltype(auto) V`): the type is
  deduced from the argument;
- deduced return types: each later `return` must deduce the same type,
  else "'auto' in return type deduced as 'X' here but deduced as 'Y' in
  earlier return statement";
- generic lambdas (WP-075).

A function with a deduced return type that is used before its type is
known has its body **instantiated immediately**. That is not the
immediate context: errors are hard.

### 6.4 Explicit args and default arguments

- **Default template arguments of function templates** are used only for
  parameters that are neither deduced nor given explicitly. They are
  substituted in order and may name earlier parameters.
- **Default function arguments** are instantiated at the call that uses
  them, under the frame `DefaultFunctionArgumentInstantiation` (a hard
  error). The note is "in instantiation of default function argument
  expression for 'f<int>' required here". The instantiated argument is
  cached per instantiated `ParmVar` in `default_args`.

### 6.5 Integration with overload resolution; when bodies are instantiated

S3O calls `deduce_call` for each function template in a candidate set.
- `Ok(spec)` gives a candidate whose parameter types are the
  specialization's.
- `Err(failure)` gives a non-viable candidate that keeps the failure for
  its note (§10.4).
- Nothing is cached across calls beyond the memoized specialization
  decls. A per-`(ft, argument-type signature)` deduction cache is a
  deferred measurement item (Q13).

`mark_referenced(f, loc)` is clang's `MarkFunctionReferenced`, applied
after selection and on any odr-use:
- **In a dependent context** it does nothing.
- **`constexpr`, `consteval` or deduced-return specializations** are
  instantiated **immediately**, under the frame `TemplateInstantiation`
  with poi `loc`. This keeps clang's diagnostic order and makes the body
  ready for the Oven (Q7).
- **Every other specialization** is pushed onto `pending`. It is
  instantiated at end of TU, or earlier through `ensure_body`.
- **Explicit-instantiation-declaration members** are queued only if they
  are inline or `constexpr`.

### 6.6 CTAD in M3 ([over.match.class.deduct])

On the first use of CTAD for a class template `X`, the guide set is
built and cached in `guides[X]`:
- one implicit guide per constructor of the primary pattern;
- the copy-deduction candidate `X(X<T…>) -> X<T…>`;
- every namespace-scope `CXXDeductionGuide` declared so far.
Building it uses the `BuildingDeductionGuides` frame.

The set then goes through overload resolution with the initializer as
the arguments. The result type is the guide's return type, after
substitution. If several best guides tie, the [over.match.best]/2.10–12
tie-breakers apply.

The M3 exit headers need only the user-declared guides:
`array(_Tp, _Up...)`, the `span` guides and the `basic_string_view`
guides.

### 6.7 Partial ordering ([temp.func.order], [temp.deduct.partial])

`more_specialized(f1, f2, ctx, nargs) -> Option<DeclId>`. `ctx` is
`Call | Conversion | Address | Other`.

1. **Synthesize unique values** for each of fᵢ's params:
   - a type param becomes `TemplateTypeParm(SYNTH_DEPTH = 0x7FFE, idx)`,
     a synthesized-param decl per template;
   - a value param becomes a `SubstNonTypeTemplateParmExpr` over an
     opaque value;
   - a template param becomes a synthesized template name.
   This is deviation D6.
2. **Pick the types.** For `Call`, only the parameters that have explicit
   call args (the first `nargs`). For `Conversion`, the return types. For
   `Address`, the function types.
3. **Adjust** ([temp.deduct.partial]/5–7): remove references and
   top-level cv, but remember for the /9 tie-breaker which side was a
   reference and how it was qualified.
4. **Deduce both ways.** `f1` is at least as specialized as `f2` if
   deducing f2's params from f1's synthesized types succeeds for every
   pair. Packs follow /8–11. The last tie-breaker for packs is
   [temp.func.order]/5: a function without a trailing pack wins.
5. **Constraints.** If both directions succeed and the template-heads are
   equivalent, the more constrained one wins
   ([temp.func.order]/6, P2113; subsumption §8.4).
6. **Result.** `Some(winner)` or `None` (ambiguous).

`po_cache: (f1, f2, ctx, nargs) → Option<DeclId>` is valid because
templates are immutable.

### 6.8 Equivalence and redeclaration ([temp.over.link])

Two function templates are the same entity when:
- their template-heads are equivalent: the same kinds and types per
  parameter, and equivalent constraints (§8.2 normal forms that are
  identical); and
- their function types are equivalent, comparing dependent expressions by
  `ProfileId` (types.md §7.2).

This equivalence is used for:
- out-of-line definitions of member templates;
- redeclarations;
- explicit specialization targets;
- friend template matching.

"Functionally equivalent but not equivalent" is ill-formed NDR. Clang and
kiln treat it as a different entity.

### 6.9 Explicit specialization and instantiation of function templates

For `template<> R f<args>(params)` or `template<> R f(params)`, and for
`template R f(params);`:
1. Collect the function templates named `f` in the scope.
2. Deduce each one from the declared function type ([temp.deduct.decl]).
3. Take the most specialized of those that succeed (§6.7).
4. If none or several remain, the errors are clang's: "no function
   template matches function template specialization 'f'" and
   "function template specialization 'f' ambiguously refers to more than
   one function template".

The `SpecKind` transitions are those of §4.5.

### 6.10 `TemplateParamObject` (class-type NTTPs)

- A class-type NTTP argument is a converted constant expression. The
  Oven evaluates it with `EvalKind::TemplateArg` and **freezes** the
  result into a `ConstValueId`.
- Template-argument equivalence ([temp.type]/2) is structural:
  - members compare recursively;
  - floating-point values compare bit-identically (clang);
  - pointers and references compare by entity and path.
- **Requirement R-OVEN-2.** `consts()` must hash-cons frozen aggregate
  values. Equivalent values then get the same `ConstValueId`, and
  canonical `ArgListId` equality is exactly equivalence.
- `param_objects[(type, value)]` yields a single `TemplateParamObject`
  decl, of type `const T`, for each value. Every use of the parameter in
  the instantiation is a `DeclRefExpr` to it. Its address is the same
  across the TU, and it mangles as `_ZTAX…` (WP-079).
- Pointer and reference NTTPs, including `template<auto V>`, use the same
  `ConstValue` path representation. Floating-point NTTPs compare by bits.

## 7. Packs

### 7.1 Representation

| Entity | Representation |
|---|---|
| template parameter pack | the `pack` bit of the param decl and of `CanonTemplateTypeParm` |
| pack argument | `TemplateArg::Pack(ListId)` holding the canonical element args |
| function parameter pack (pattern) | a `ParmVar` of `PackExpansion` type |
| function parameter pack (instantiated) | N `ParmVar`s; a reference that cannot expand yet is a `FunctionParmPackExpr` |
| pack substituted while an inner pattern is still unexpanded | `SubstTemplateTypeParmPack`, `SubstNonTypeTemplateParmPackExpr` |
| using-declaration pack | a `UsingPack` decl |

An unexpanded pack is the `UNEXPANDED_PACK` bit on types, expressions and
template args. Every *full-expression*, declarator, base-specifier and
template argument outside an expansion context is checked. A violation is
"expression contains unexpanded parameter pack 'T'" (or "declaration
type …", or "base type …"). The check walks the node, pruning on the
flag, to name the first pack.

### 7.2 Expansion: `expand(pattern, site) -> Result<Expansion, SubstFail>`

1. **Collect the unexpanded packs** in `pattern`, pruning on
   `UNEXPANDED_PACK`. Patterns are immutable, so the result is cached per
   pattern node in `pack_refs: FxHashMap<NodeRef, SmallVec<[PackRef; 2]>>`.
2. **Find each pack's length.** A `Substitute` level with a `Pack` arg
   gives its element count. An instantiated function parameter pack gives
   its `ParmVar` count. If any pack belongs to a `Retain` level, the
   pattern cannot be expanded yet: substitute it once without
   `pack_index`, and give `PackExpansion(subst_pattern)`.
3. **Check the lengths.** They must all be equal. Otherwise: "pack
   expansion contains parameter packs 'T' and 'U' that have different
   lengths (2 vs. 3)". This is SFINAE-able.
4. **Expand.** For k in 0..N, substitute the pattern with
   `pack_index = Some(k)`. Restore the previous index afterwards. Inner
   expansions keep their own packs untouched.
5. **Splice** the N results into the enclosing list: template args, call
   args, initializer list, bases, mem-initializers, `UsingPack`
   members, or capture list.

In deduction, a trailing P pack expansion matches the remaining A
elements one by one. Each pack in the pattern collects its own
`TemplateArg::Pack`. A partially explicit pack is extended with the
deduced elements (§6.1). A pack that is not deduced and was not given
explicitly is the empty pack.

### 7.3 Folds ([expr.prim.fold], [temp.variadic]/11)

With elements `E1..EN`:

| Form | Expansion |
|---|---|
| unary right fold `(E op ...)` | `E1 op (… op (EN-1 op EN))` |
| unary left fold `(... op E)` | `((E1 op E2) op …) op EN` |
| binary right fold `(E op ... op I)` | the right fold with `I` innermost |
| binary left fold `(I op ... op E)` | the left fold with `I` innermost |

**N = 0 in a unary fold:**
- `&&` gives `true`;
- `||` gives `false`;
- `,` gives `void()`;
- any other operator is an error: "unary fold expression has empty
  expansion for operator '+' with no fallback value".

**Building.** Each step goes through `act_binary`, which does overload
resolution. The whole result is wrapped in `ParenExpr`, as clang's
`TransformCXXFoldExpr` does; the golden `packs/fold-dump.cpp` pins this
down.

**Deep trees.** A fold over N elements yields a tree N deep.
- Requirement R-WALK: every consumer that walks expression trees
  (Substituter, Oven lowering, codegen, dumper) handles binary-operator
  chains of depth ≥ 65,536. It either iterates along the spine or stays
  within the stack guard's budget.
- The test is `stress/fold-65536.cpp` (Q16).

### 7.4 `sizeof...` and pack indexing

**`sizeof...(P)`.** After substitution it is a `SizeOfPackExpr` with the
length stored. It is not dependent. Its dump matches clang's.

**Pack indexing (P2662) lands in M3**: clang 19 is the oracle, and the
cost is small.
- `T...[I]` (type) and `p...[I]` (expression). `I` is a converted
  constant expression of type `size_t`.
- After substitution:
  - the type gives the selected element, keeping `PackIndexing` as sugar;
  - the expression gives a `PackIndexingExpr` with the selected element.
- An index out of range: "invalid index 5 for pack 'T' of size 2". This
  is SFINAE-able.
- A pack-index is a non-deduced context.

`__type_pack_element` (§9.3) shares its implementation.

## 8. Concepts

### 8.1 Entities

- **`ConceptDecl`** holds `{params, constraint: ExprId}`.
- **A concept-id `C<A…>`** checks and converts its args (§4.2), then
  builds `ConceptSpecializationExpr`:
  - it refers to `ImplicitConceptSpecialization`, which holds the
    converted args;
  - it carries `sat: Option<SatId>`;
  - a non-dependent concept-id is checked immediately, and its value
    (`bool`) is fixed at construction.
- **The associated constraints of a templated entity**
  ([temp.constr.decl]/3) are the conjunction, in this order, of:
  1. the template-head's requires-clause;
  2. each parameter's immediately-declared constraint, in order;
  3. the trailing requires-clause.
  For abbreviated templates, the invented parameters come first in that
  order, as clang does.

### 8.2 Normalization and atomic-constraint identity

```rust
pub struct NormId(NonZeroU32);
pub enum NormNode {                         // 12 B, hash-consed in NormTable
    Atomic { expr: ExprId, mapping: ArgListId },   // ExprId of the expression as written; mapping canonical
    And(NormId, NormId), Or(NormId, NormId),
}
```

`normalize(e, mapping)` follows [temp.constr.normal]:
- `(E)` normalizes to `E`.
- `E1 && E2` gives `And`; `E1 || E2` gives `Or`.
- A concept-id `C<A…>` normalizes `C`'s constraint expression with the
  parameter mapping substituted by `A…`. The frame is
  `ParameterMappingSubstitution`; a failure there makes the program
  ill-formed (clang's error).
- Anything else gives an `Atomic` node.

**Identity** ([temp.constr.atomic]/2). Two atomic constraints are
identical iff:
- they are the *same expression in the source*, i.e. the same `ExprId`;
  and
- their mappings are equivalent, i.e. the same canonical `ArgListId`
  (comparing dependent args by `ProfileId`).

Hash-consing therefore makes identical atoms the **same `NormId`**.
`normal_forms: DeclId → NormId` caches each entity's associated
constraints, built under the `ConstraintNormalization` frame.

Fold-expanded constraints (P2963) are M4. Until then, the immediately
declared constraint of a constrained pack `C... Ts` is the fold
`(C<Ts> && ...)`, treated as one atom (clang 19).

### 8.3 Satisfaction ([temp.constr.sat])

`check_satisfaction(entity, args, loc) -> SatId` runs under the frame
`ConstraintsCheck`. It evaluates the normal form:

- **Atomic.** Substitute `args` into `mapping` and then into `expr`,
  under `ConstraintSubstitution` (SFINAE).
  - A substitution failure means not satisfied, with the captured
    diagnostic recorded.
  - Otherwise the expression must have type exactly `bool`. Anything else
    is the hard error "atomic constraint must be of type 'bool' (found
    'int')".
  - The value is evaluated by the Oven with `EvalKind::Constraint` (a new
    `EvalContext` kind; Q5).
  - A non-constant result is clang's error "substitution into constraint
    expression resulted in a non-constant expression".
- **`And` and `Or`** short-circuit from left to right.

The result is stored:

```rust
pub struct Satisfaction { ok: bool, records: Vec<SatRecord> }   // records: only when !ok, for notes
pub enum SatRecord { False { atom: NormId, subst: ExprId }, SubstFailed { atom: NormId, diag: Diagnostic },
                     Requirement { req: ExprId, idx: u16, diag: Option<Diagnostic> } }
```

**Two cache levels.** Caching is allowed: [temp.constr.atomic]/3 makes a
change in result ill-formed NDR.
1. `sat_cache: (entity or concept, canonical args) → SatId`. This is
   clang's behaviour.
2. `atom_cache: (NormId of the atom, canonical substituted mapping) →
   bool`. Atoms from shared sub-concepts (`same_as`, `convertible_to`)
   are thus evaluated once across *all* concepts and overloads that use
   them.

`Satisfaction` records are kept for the TU's life. They are needed for
the notes, and they are metered as `templ.satisfaction`.

### 8.4 Subsumption ([temp.constr.order])

`subsumes(P, Q)` holds iff every disjunctive clause of DNF(P) subsumes
every conjunctive clause of CNF(Q). A clause is a sorted
`SmallVec<[NormId; 4]>` of atoms. Clause sets are built lazily, with a
cap of 65,536 clauses (D5).
- `subsume_cache: (NormId, NormId) → bool`.
- It is used by:
  - the overload tie-breaker ([over.match.best]/2.6 via
    [temp.func.order]/6);
  - partial-specialization ordering (§4.4);
  - P0522 template template argument matching.

### 8.5 Requires-expressions ([expr.prim.req])

In a non-dependent context the expression is evaluated at construction.
Otherwise it is evaluated at substitution.

Its requirements are processed **in order**, each under
`RequirementInstantiation` (SFINAE). The first failure makes the result
`false` and is recorded for the notes.

| Requirement | Satisfied iff |
|---|---|
| simple `E;` | substituting `E` succeeds (it is not evaluated) |
| type `typename T::x;` | substitution forms a valid type |
| compound `{E} noexcept -> C<A…>;` | `E` substitutes; with `noexcept`, `E` cannot throw; with a type-constraint, `C<decltype((E)), A…>` is satisfied |
| nested `requires B;` | `B` is satisfied (§8.3), under `NestedRequirementConstraintsCheck` |

A `requires`-expression outside any template whose requirement is
ill-formed is a hard error ([expr.prim.req.general]/5, clang).

The expression is a `bool` prvalue. `RequiresExpr` stores the value plus
the index of the failed requirement.

### 8.6 Constrained parameters, constrained `auto`, abbreviated templates

- **`C<A…> T`.** The type parameter gets the immediately-declared
  constraint `C<T, A…>`, stored as its type-constraint
  (`ConceptReference` in clang's dump).
- **`C auto` in a parameter.** This creates an invented parameter with
  that constraint (§2.6).
- **`C auto x = e;`** checks `C` against the deduced type (§6.3).
- **A constrained NTTP `C auto V`.** The constraint is checked when the
  argument is converted.

### 8.7 Note chains (clang's exact texts; used by §10.4)

`SatRecord`s are rendered recursively. For a concept-id the note is
"because 'char' does not satisfy 'C'", and the concept's own records
follow. For an atom:

| Situation | Note |
|---|---|
| an atom evaluated to false | "because 'sizeof(char) > 4' (1 > 4) evaluated to false" (the values in parentheses are printed for binary comparisons whose operands are both literals after evaluation, as clang's `note_atomic_constraint_evaluated_to_false_elaborated`) |
| an atom whose substitution failed | "because substituted constraint expression is ill-formed: <diag>" |
| a `requires` simple or compound requirement failed | "because 'E' would be invalid: <diag>" |
| a type requirement failed | "because 'typename T::x' would be invalid: <diag>" |
| a `noexcept` requirement was not met | "because 'E' may throw an exception" |
| a return-type constraint failed | "because type constraint 'C<int>' was not satisfied:" followed by the nested chain |
| a nested requirement failed | "because 'B' evaluated to false" (or the nested chain) |
| `\|\|` | notes for *both* operands, left first |

## 9. Builtins table

The set below is every builtin that appears in the clang-preprocessed M3
exit headers (`-std=c++26 -stdlib=libc++`, libc++ 19), extracted with
`__has_builtin` (§13.3).

**One source of truth.** `kiln_sema::templ::builtins::TABLE` lists, for
each name: its kind, arity, implementation status, and whether it is
constexpr.
- kiln_pp's `__has_builtin(X)` answers from the **implemented** column
  (Q10).
- libc++ then falls back to its library implementations for any trait
  kiln has not implemented yet. That allows staging.

**Notation.** "complete" means the operand must be complete, or an array
of unknown bound, or `void`. Otherwise the error is "incomplete type 'X'
used in type trait expression". Each "complete" check calls
`require_complete`, so it may instantiate.

**The query column.**
- `tc.*` are `TypeContext` classification queries (types.md).
- `cls.*` are the S3C record properties (`RecordProps`).
- `abi.*` are `kiln_abi` layout queries.
- `init_check(T, Args, kind)` is the unevaluated initialization check:
  it builds the initialization in an unevaluated SFINAE context
  ([meta.unary.prop] "`T t(declval<Args>()...);` is well-formed") and
  returns `{ok, trivial, nothrow}`.
- `conv_check(F, T)` is the same for an implicit conversion of
  `declval<F>()` to `T`.

### 9.1 Type predicates (`TypeTraitExpr`, `bool` prvalue, unevaluated operands)

| Builtin | True iff | Query |
|---|---|---|
| `__is_same(T,U)` | the canonical types are equal (cv included) | `tc.canonical` |
| `__is_integral(T)` | `bool`, a character type or an integer type (incl. `__int128`); not an enum | `tc.is_integral` |
| `__is_signed(T)` | an arithmetic type that is signed; floating types are true; enums are false | `tc.is_signed_arith` |
| `__is_unsigned(T)` | an unsigned integer type (incl. `bool`); enums are false | `tc.is_unsigned_int` |
| `__is_fundamental(T)` | arithmetic, `void` or `nullptr_t` | `tc` |
| `__is_compound(T)` | `!__is_fundamental` | `tc` |
| `__is_scalar(T)` | arithmetic, enum, pointer, member pointer or `nullptr_t` | `tc` |
| `__is_object(T)` | not a function, reference or `void` | `tc` |
| `__is_array(T)` | an array, including unknown bound | `tc` |
| `__is_pointer(T)` | a pointer (not a member pointer) | `tc` |
| `__is_lvalue_reference(T)` / `__is_rvalue_reference(T)` / `__is_reference(T)` | the matching kind | `tc` |
| `__is_member_pointer(T)` / `__is_member_object_pointer(T)` / `__is_member_function_pointer(T)` | the matching kind | `tc` |
| `__is_function(T)` | a function type | `tc` |
| `__is_const(T)` / `__is_volatile(T)` | the top-level qualifier (false for references and functions) | `QualType` bits |
| `__is_enum(T)` / `__is_scoped_enum(T)` | an enum / a scoped enum | `tc`, Enum decl flag |
| `__is_class(T)` / `__is_union(T)` | a non-union class / a union | `tc` |
| `__is_referenceable(T)` | an object type, a function without cv or ref qualifiers, or a reference | `tc` |
| `__is_empty(T)` | complete; a non-union class with no non-static members other than zero-width bit-fields, no virtual functions or virtual bases, and only empty bases | `cls.empty` |
| `__is_polymorphic(T)` | complete; declares or inherits a virtual function | `cls.polymorphic` |
| `__is_abstract(T)` | complete; has a pure virtual function, own or inherited and not overridden | `cls.abstract` |
| `__is_final(T)` | complete; a class marked `final` | decl flag |
| `__is_aggregate(T)` | complete; an array, or a class per [dcl.init.aggr]/1 | `cls.aggregate` |
| `__is_trivial(T)` | complete; scalar, a trivial class, or an array of those | `cls.trivial` |
| `__is_trivially_copyable(T)` | complete; [basic.types.general]/9 | `cls.trivially_copyable` |
| `__is_standard_layout(T)` | complete; [basic.types.general]/9 | `cls.standard_layout` |
| `__is_pod(T)` | complete; trivial and standard-layout | `cls` |
| `__has_virtual_destructor(T)` | complete; its destructor is virtual | `cls` |
| `__has_unique_object_representations(T)` | complete; [meta.unary.prop]: no padding bits; floating types are false; member pointers per the Itanium ABI | `abi.record_layout` + `tc` |
| `__is_trivially_equality_comparable(T)` | clang 19: trivially copyable, unique object representations, and no user `operator==` (or a defaulted one whose members are all trivially comparable) | `cls` + `abi` + lookup |
| `__is_trivially_relocatable(T)` | clang 19: trivially copyable, or a class with `[[clang::trivial_abi]]` that is not volatile | `cls` |
| `__is_base_of(B,D)` | both are classes (not unions), `D` is complete when the two differ, and `B` is `D` or a base of `D` (cv ignored) | `cls.bases` (S3C) |
| `__is_convertible(F,T)` / `__is_nothrow_convertible(F,T)` | `conv_check(F,T).ok` (and `.nothrow`) | `conv_check` |
| `__is_constructible(T,A…)` / `__is_trivially_constructible` / `__is_nothrow_constructible` | `init_check(T, A…, Direct)`: `.ok`, `.ok && .trivial`, `.ok && .nothrow`. A reference `T` with a single argument does **not** count as constructible when the reference would bind to a temporary created by an implicit conversion (clang 19) | `init_check` |
| `__is_assignable(T,U)` / `__is_trivially_assignable` / `__is_nothrow_assignable` | `declval<T>() = declval<U>()` is well-formed (unevaluated), and is trivial / cannot throw | `init_check(Assign)` |
| `__is_destructible(T)` / `__is_nothrow_destructible(T)` / `__is_trivially_destructible(T)` | [meta.unary.prop] (`void`, functions and arrays of unknown bound are false) | `cls.dtor` |

Two more behave the same way but are only probed, not used, by the M3
headers. They are implemented in M3 because they cost one row each:
`__reference_constructs_from_temporary` and `__is_layout_compatible`.

### 9.2 Type transforms (`UnaryTransform` types)

libc++ 19 spells these as `using __remove_cv_t = __remove_cv(_Tp);`.
- The type is `UnaryTransform{kind, operand}` sugar whose canonical type
  is the result. It prints as clang does, e.g. `__remove_cv(int const)`.
- A dependent operand gives a dependent `UnaryTransform`. Substitution
  recomputes it (§5.3).

| Builtin | Result |
|---|---|
| `__remove_cv`, `__remove_const`, `__remove_volatile` | the top-level qualifiers removed (cv of a reference is unchanged) |
| `__remove_reference_t` | the referenced type |
| `__remove_cvref` | `remove_cv(remove_reference(T))` |
| `__remove_pointer` | the pointee (a pointer with cv drops cv too, per [meta.trans.ptr]) |
| `__remove_extent`, `__remove_all_extents` | one, or all, array dimensions removed |
| `__add_pointer` | `remove_reference(T)*` if referenceable or `void`; otherwise `T` |
| `__add_lvalue_reference`, `__add_rvalue_reference` | `T&` / `T&&` if referenceable (reference collapsing); otherwise `T` |
| `__decay` | [meta.trans.other] decay |
| `__make_signed`, `__make_unsigned` | [meta.trans.sign]. An enum maps to the integer type with its size and rank order. The operand must be integral (not `bool`) or an enum; otherwise "'make_signed' cannot be applied to 'bool'"-style errors |
| `__underlying_type` | the enum's underlying type. The enum must be complete; non-enums give clang's error |

### 9.3 Builtin templates (`BuiltinTemplate` decls)

These are injected into the TU's global scope at startup, like clang's
`__make_integer_seq` and `__type_pack_element` decls.

| Builtin | Parameters | Result |
|---|---|---|
| `__make_integer_seq<Seq, T, N>` | `template<template<class U, U...> class Seq, class T, T N>` | `Seq<T, 0, 1, …, N-1>`, through `get_class_spec`. `N < 0` is "integer sequences must have non-negative sequence length". The result is `TemplateSpecialization` sugar (clang). This is O(N) argument construction, with no recursion |
| `__type_pack_element<I, Ts...>` | `template<size_t I, class... Ts>` | `Ts...[I]`. It shares §7.4; an out-of-range `I` is "__type_pack_element index out of bounds" |

`__builtin_common_type` does not exist in clang 19, and libc++ 19 does
not use it, so it is not in M3.

### 9.4 Other trait-like operators

| Builtin | Semantics | Query |
|---|---|---|
| `__array_extent(T, I)` | `ArrayTypeTraitExpr`, of type `size_t`: the bound of dimension `I`, or 0 | `tc` |
| `__builtin_is_constant_evaluated()` | the Oven returns `true` in a manifestly constant-evaluated context. Codegen emits `false`. Sema warns "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression" where clang does. Owned by WP-063 | Oven `EvalContext` |
| `__builtin_constant_p(e)` | the Oven returns `true` if `e` folds to a constant (clang's rules for pointers and strings). Otherwise codegen lowers it through `llvm.is.constant` | Oven, codegen |

### 9.5 `__builtin_*` functions used by the M3 headers

These are declared by WP-063's builtin declaration table, with custom
type-checking where marked †. "Oven" means the builtin is evaluable in
constant expressions in clang 19 and therefore in the Oven. WP-074
verifies every Oven bit with a `static_assert` probe against clang 19 and
records it in `TABLE`.

| Group | Builtins | Oven | Lowering |
|---|---|---|---|
| object model | `__builtin_addressof`†, `__builtin_launder`†, `__builtin_bit_cast`† (keyword form: `T` and the operand must be trivially copyable and the same size; `abi` layout gives the bytes), `__builtin_unreachable` | yes (`unreachable`: a non-constant diagnostic) | intrinsic / no-op |
| allocation | `__builtin_operator_new`, `__builtin_operator_delete` | yes in clang 19; the Oven side is WP-061b's (transient allocation) | `operator new`/`delete` call |
| bits | `__builtin_clz`/`l`/`ll`, `__builtin_ctz`/`l`/`ll`, `__builtin_popcount`/`l`/`ll`, `__builtin_clzg`†/`ctzg`†/`popcountg`† (type-generic, with an optional fallback argument), `__builtin_bswap16`/`32`/`64` | yes | `llvm.ctlz`/`cttz`/`ctpop`/`bswap` |
| memory and strings | `__builtin_memcmp`, `__builtin_memcpy`, `__builtin_memmove`, `__builtin_memchr`, `__builtin_char_memchr`, `__builtin_strlen`, `__builtin_wcslen`, `__builtin_wmemchr`, `__builtin_wmemcmp` | yes (for trivially copyable element types) | libcall or `llvm.mem*` |
| float classification | `__builtin_isfinite`, `__builtin_isinf`, `__builtin_isnan`, `__builtin_isnormal`, `__builtin_signbit`†, `__builtin_isgreater`, `isgreaterequal`, `isless`, `islessequal`, `islessgreater`, `isunordered` | yes for `isfinite`/`isinf`/`isnan`/`isnormal`; no for `signbit` and the six comparisons (the clang 19 probe rejects them) | `fcmp` / `llvm.is.fpclass` |
| float constants | `__builtin_huge_val`/`f`/`l`, `__builtin_nan`/`f`/`l`, `__builtin_nans`/`f`/`l` | yes | constants |
| math | `__builtin_exp`/`exp2`/`expm1`/`frexp`/`ldexp`/`pow`/`scalbn`/`scalbln`, each with the `f` and `l` forms | no (clang 19) | libcall / LLVM intrinsic |
| source location | `__builtin_FILE`, `__builtin_FILE_NAME`, `__builtin_FUNCTION`, `__builtin_LINE`, `__builtin_COLUMN`, `__builtin_source_location`† (keywords: a `SourceLocExpr`; in a default argument, the location of the call that uses it) | yes | constants; a private `.constant` `__impl` object per location |

**Landed (WP-088).** Every row above is implemented in sema, the Oven
and codegen, with these exceptions, each matching clang 19:
`__builtin_signbit` and the six ordered comparisons are not
constant-evaluated (the probe says clang 19 rejects them), and the math
row is sema and codegen only. `__builtin_operator_new`/`_delete` are
implemented in sema and codegen; their Oven side stays with WP-061b
(`__has_constexpr_builtin` answers 0). Probes:
`tests/conformance/expr.const/builtins-*.cpp` (constant evaluation,
`static_assert` against clang 19, plus the compile-fail and run tests),
`tests/codegen/builtins_*.cpp` (IR-diff), and
`bench/oven/builtins-{memcpy,strlen,popcount}.cpp`. Oven behaviour
(byte-level `memcpy`/`memmove` on trivially copyable element arrays,
`bit_cast` through the ABI layout, source-location objects) is in
spec/oven.md "Builtins".

**Source location.** Sema computes the values once, when it builds the
`SourceLocExpr` (spec/ast.md §5.8): `LINE`/`COLUMN` from the presumed
location, `FILE` as written and `FILE_NAME` as its last path component,
`FUNCTION` from the enclosing function's name (empty at namespace scope),
and `__builtin_source_location()` as a frozen `std::source_location::__impl`
built from that record's layout. When a default argument containing a
`SourceLocExpr` is used by a call, sema records the call's values on the
`CXXDefaultArgExpr`; the Oven and codegen read them from the outermost
such default argument in scope. `std::source_location::__impl` must be a
complete, standard-layout, non-union class without bases whose only
fields are `_M_file_name`, `_M_function_name` (`const char *`), `_M_line`
and `_M_column` (integral); otherwise clang's
`sema_std_source_location_impl_not_found`/`_malformed` errors.

The counts are 49 type predicates (§9.1), `__array_extent`, 15 transforms,
2 builtin templates and 76 `__builtin_*` names (2 of them in §9.4). That gives 143
builtins in total (§13.3). `TABLE` also holds the 6 source-location
names, for 82 function rows.

## 10. Diagnostics

### 10.1 Instantiation frames and their notes

```rust
pub struct InstFrame {          // 20 B
    kind: FrameKind,            // u8
    sfinae: Option<bool>,       // None = inherit from the enclosing frame (§5.7)
    entity: DeclId, args: Option<ArgListId>, poi: Loc,
    meter: MeterIdx,            // u32 into the report's open-frame stack
}
```

| `FrameKind` | SFINAE | Note (clang 19 text; `%0` is printed with the template args) |
|---|---|---|
| `ClassInstantiation` | hard | in instantiation of template class '%0' requested here |
| `FunctionInstantiation` | hard | in instantiation of function template specialization '%0' requested here |
| `MemberFunctionInstantiation` | hard | in instantiation of member function '%0' requested here |
| `StaticDataMemberInstantiation` | hard | in instantiation of static data member '%0' requested here |
| `VarTemplateInstantiation` | hard | in instantiation of variable template specialization '%0' requested here |
| `MemberClassInstantiation` | hard | in instantiation of member class '%0' requested here |
| `MemberEnumInstantiation` | hard | in instantiation of enumeration '%0' requested here |
| `DefaultMemberInitializerInstantiation` | hard | in instantiation of default member initializer '%0' requested here |
| `DefaultTemplateArgumentInstantiation` | inherit | in instantiation of default argument for '%0' required here |
| `DefaultFunctionArgumentInstantiation` | hard | in instantiation of default function argument expression for '%0' required here |
| `ExplicitTemplateArgumentSubstitution` | SFINAE | while substituting explicitly-specified template arguments into function template '%0' |
| `DeducedTemplateArgumentSubstitution` | SFINAE | while substituting deduced template arguments into function template '%0' [with …] |
| `PriorTemplateArgumentSubstitution` | inherit | while substituting prior template arguments into non-type template parameter [with …] |
| `DefaultTemplateArgumentChecking` | inherit | while checking a default template argument used here |
| `ExceptionSpecInstantiation` | hard | in instantiation of exception specification for '%0' requested here |
| `ConstraintsCheck` | inherit | while checking constraint satisfaction for template '%0' required here |
| `ConceptCheck` | inherit | while checking the satisfaction of concept '%0' requested here |
| `ConstraintSubstitution` | SFINAE | while substituting template arguments into constraint expression here |
| `ConstraintNormalization` | inherit | while calculating associated constraint of template '%0' here |
| `ParameterMappingSubstitution` | inherit | while substituting into concept arguments here; substitution failures not allowed in concept arguments |
| `RequirementInstantiation` | SFINAE | in instantiation of requirement here |
| `NestedRequirementConstraintsCheck` | SFINAE | while checking the satisfaction of nested requirement requested here |
| `BuildingDeductionGuides` | inherit | while building implicit deduction guide first needed here |
| `LambdaSubstitution` | inherit | while substituting into a lambda expression here |
| `DeclaringSpecialMember`, `DefiningSynthesizedFunction`, `RewritingOperatorAsSpaceship` | — | owned by S3C/S3O; listed here so that they count toward depth and elision |

The texts are taken from clang 19's `DiagnosticSemaKinds.td`. The goldens
are authoritative: WP-070 checks each of them in `templates/diag/`.

The non-instantiation kinds are the "while …" rows. They do not count
toward `-ftemplate-depth`. They are clang's `NonInstantiationEntries`.

### 10.2 When the stack is printed, and elision

**When** (clang's `PrintContextStack` and
`LastEmittedCodeSynthesisContextDepth`):
- The stack is printed with an error or warning when the stack is
  non-empty and its depth differs from `last_emitted`. After printing,
  `last_emitted` is set to the depth.
- Popping the frame at which the stack was last printed resets
  `last_emitted` to 0.
- So two errors at the same depth print the stack only once, as in
  clang.
- Notes never trigger printing.
- Only a diagnostic that will print takes the notes: `Sema::emit` asks
  `DiagEngine::would_emit` first (spec/diag.md §7.4), so a dropped
  duplicate or an ignored warning does not set `last_emitted` and leave
  the next real error without its context.

**Deduplication.** `Sema::emit` sets the diagnostic's dedup context to
`InstantiationStack::context_key()`, a hash of every frame's `(kind,
entity, args, poi)` (0 only for an empty stack). The same error in two
instantiations (`S<int>`, `S<long>`) prints twice, each with its notes;
see spec/diag.md §7.4 for the full rule (WP-087).

**Elision.** `-ftemplate-backtrace-limit=N` defaults to 10; 0 means no
limit. With F frames and F > N > 0:
- frames `[0, ⌈N/2⌉)` are printed innermost first;
- then the note "(skipping K contexts in backtrace; use
  -ftemplate-backtrace-limit=0 to see all)", at the location of frame
  `⌈N/2⌉`, where `K = F − N`;
- then frames `[F − ⌊N/2⌋, F)`.

Verified against clang 19 with `R<N>::v = R<N-1>::v`, `R<2000>`, limit 4
(the scratch test `d1.cpp`):
- the error is at depth 1025;
- the notes show `R<976>`, `R<977>`, "skipping 1021", `R<1999>`, `R<2000>`.

### 10.3 Depth and stack limits

**`-ftemplate-depth=N`** (default 1024). The check runs when an
instantiation-kind frame is pushed: the error fires when the number of
instantiation-kind frames exceeds N. The fatal error is "recursive
template instantiation exceeded maximum depth of 1024", with the note
"use -ftemplate-depth=N to increase recursive template instantiation
depth". Fatal means kiln_diag stops the TU.

**Stack guard (D1).**
- `StackGuard { base: usize, limit: usize }` records the address of a
  local at sema-thread start. A pointer-to-integer cast is safe Rust.
- `remaining()` compares that with the address of a local in the current
  frame. With less than 1 MiB remaining, the fatal error is "kiln:
  stack exhausted during template instantiation; reduce nesting or raise
  -fkiln-stack-size", followed by the instantiation stack.
- The sema thread is spawned with `-fkiln-stack-size`, default 256 MiB.
  This is reserved address space; pages are committed lazily.
- `stress/template-depth-1024.cpp` asserts that depth 1024 of the worst
  bench pattern uses less than ¼ of the default stack.

*Landed (WP-086).*
- `kiln_sema::stack_guard`: `StackGuard { base, size }` is installed per
  thread (`StackGuard::install(size)`, a thread-local) by the driver's
  `main` on the compiling thread; a thread without one (unit tests,
  library callers) never trips. The red zone is 1 MiB (`RED_ZONE`).
- **Default 1 GiB** (`DEFAULT_STACK_SIZE`; Q4 decision below), not
  256 MiB: one instantiation level of the `R<N>::v` chain costs up to
  ~300 KiB in a debug build, so 256 MiB does not reach the default depth
  1024 there. `-fkiln-stack-size=N[K|M|G]` (last valid one wins; values
  below 4 MiB are raised to 4 MiB; a malformed value is
  `drv_invalid_value`).
- The fatal is `sema_stack_exhausted`: "kiln: stack exhausted during
  {parsing|template instantiation|constant evaluation}; reduce nesting
  or raise -fkiln-stack-size", where the phase is the checking site:
  - **template instantiation**: `push_inst_frame` (the notes then show
    the instantiation stack, with the frame being pushed on top) and
    `Substituter::{expr,ty,stmt}`;
  - **parsing**: the recursive parser rules (external declarations,
    declarators, initializer lists, template-argument lists, member
    declarations, statements, cast-expressions, the tentative
    declarator, `scan_name_at`). After a trip the parser sees end of
    file, so it unwinds without further diagnostics;
  - **constant evaluation**: `oven_bridge` before each evaluation.
- Stress tests (spawned binary, so the guard is installed):
  `stress/stack-guard-parens.cpp` (50,000 nested parentheses under
  `-fkiln-stack-size=8M`) and `stress/stack-guard-template.cpp`
  (unterminated `R<N - 1>::v` recursion under `-ftemplate-depth=100000
  -fkiln-stack-size=32M`; the phase it reports is not pinned: whether the
  frame push or the Oven entry for `N - 1` trips first depends on frame
  sizes). Both are `skip(clang)`: clang stops at its
  bracket depth (256) and its own depth/stack handling instead.
- With the default 1 GiB, 50,000 parentheses parse without a trip
  (debug 0.11 s); depth 100000 hits `-ftemplate-depth` in release and
  the guard (during constant evaluation, at depth ~22,500) in debug.
- `stress/template-depth-1024.cpp` is not written; the depth-1024 bench
  entry `templates_deep.cpp` runs on the default stack.

`-fconstexpr-depth` and the Oven budgets (oven.md) are separate limits.

### 10.4 Notes for failed deductions

These are attached to "no matching function for call to 'f'" and to the
ambiguity errors.

| `Tdk` | Note |
|---|---|
| `Inconsistent` (types / values / templates) | candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'double') — or `values … (1 vs. 2)` / `templates …` |
| `Incomplete` | candidate template ignored: couldn't infer template argument 'T' |
| `IncompletePack` | candidate template ignored: deduced too few arguments for expanded pack 'T'; no argument for 2nd expanded parameter in deduced argument pack 'U' |
| `Underqualified` | candidate template ignored: cannot deduce a type for 'T' that would make 'const T' equal 'volatile int' |
| `SubstitutionFailure` | candidate template ignored: substitution failure [with T = int]: <captured diagnostic> |
| … where the failure is `enable_if<false>::type` | candidate template ignored: requirement 'is_integral<double>::value' was not satisfied [with T = double] (clang's `enable_if` special case: the failing condition is printed) |
| `DeducedMismatch` | candidate template ignored: deduced type 'X' of 1st parameter does not match adjusted type 'Y' of argument [with T = …] |
| `NonDeducedMismatch` | candidate template ignored: could not match 'vector<T>' against 'int' |
| `TooManyArguments` / `TooFewArguments` | candidate function template not viable: requires 2 arguments, but 1 was provided |
| `InvalidExplicitArguments` | candidate template ignored: invalid explicitly-specified argument for template parameter 'T' (or for the 1st template parameter) |
| `NonDependentConversionFailure` | candidate function template not viable: no known conversion from 'A' to 'B' for 1st argument |
| `ConstraintsNotSatisfied` | candidate template ignored: constraints not satisfied [with T = char], followed by the §8.7 chain |
| `InstantiationDepth` | candidate template ignored: substitution exceeded maximum template instantiation depth |

**Printing.**
- `[with T = int, U = char]` lists the deduced args. Packs print as
  `T = <int, char>`.
- Specialization names print with the as-written args, omitting any arg
  equal to its default (types.md §18).

### 10.5 Other template diagnostics (texts from clang 19; goldens are authoritative)

- "too few template arguments for class template 'X'" / "too many …";
  the note is "template is declared here".
- "template argument for template type parameter must be a type".
- "non-type template argument is not a constant expression", followed by
  the Oven note chain.
- "implicit instantiation of undefined template 'X<int>'".
- "explicit specialization of 'X<int>' after instantiation".
- "ambiguous partial specializations of 'X<int *>'".
- "class template partial specialization is not more specialized than
  the primary template". This is a warning that defaults to an error.
- "partial specialization of 'X' does not use any of its template
  parameters".
- "redefinition of default argument".
- "missing 'typename' prior to dependent type name 'T::x'".
- "use 'template' keyword to treat 'f' as a dependent template name".
- "no member named 'x' in 'A<int>'".
- "type 'int' cannot be used prior to '::' because it has no members".
- "'auto' not allowed in template parameter until C++17" (not reachable
  under C++26, but kept).

## 11. Interfaces and memory accounting

### 11.1 Files

| Crate | Path | Contents | Owner |
|---|---|---|---|
| kiln_ast | `src/templates.rs` | `TemplateTables`, `TemplateInfo`, the `AstBuilder` template constructors | 069 (constructors), 070 (tables) |
| kiln_parse | `src/templ.rs` (new); hooks in `tokens.rs` (`split_first`, `annots`), `decl.rs`, `expr.rs`, `stmt.rs` | §2 | 069 |
| kiln_sema | `src/templ/actions.rs` | parser actions, dependent-node builders, `classify_name` template variants | 069 |
| | `src/templ/{mod,args,class,pending,stack,report}.rs`, `src/templ/subst/{mod,types,exprs,stmts,decls}.rs` | §4, §5, §10, §11.3 | 070 |
| | `src/templ/{deduce,order}.rs` | §6.2, §6.7 core, used by §4.4 | 070 |
| | `src/templ/{deduce_call,funcs,ctad}.rs` | §6.1, §6.3–6.6, §6.9–6.10 | 071 |
| | `src/templ/pack.rs` | §7 | 072 |
| | `src/templ/concepts/{normal,sat,subsume,requires}.rs` | §8 | 073 |
| | `src/templ/builtins/{table,traits,transforms,builtin_templates}.rs`, `src/templ/auto.rs` | §9, §6.3 | 074 |

### 11.2 `Sema` additions

```rust
pub struct Sema<'a, 'sm> { /* M2 fields … */ pub(crate) inst: InstState }
pub(crate) struct InstState {
    stack: InstantiationStack,                 // frames: Vec<InstFrame>, non_inst: u32, last_emitted: u32
    pending: VecDeque<PendingInst>,            // PendingInst = { decl: DeclId, poi: Loc } (8 B)
    diag_mode: DiagMode,
    suppressed: FxHashMap<DeclId, Vec<Diagnostic>>,
    alias_cache: FxHashMap<(DeclId, ArgListId), QualType>,
    sat_cache: FxHashMap<(DeclId, ArgListId), SatId>,  satisfactions: Vec<Satisfaction>,
    atom_cache: FxHashMap<(NormId, ArgListId), bool>,
    norm: NormTable, normal_forms: FxHashMap<DeclId, NormId>,
    subsume_cache: FxHashMap<(NormId, NormId), bool>,
    po_cache: FxHashMap<(DeclId, DeclId, PoCtx, u16), Option<DeclId>>,
    pack_refs: FxHashMap<NodeRef, SmallVec<[PackRef; 2]>>,
    guides: FxHashMap<DeclId, ListId>,
    default_args: FxHashMap<DeclId, ExprId>,
    report: InstReport, limits: InstLimits /* depth 1024, backtrace 10 */, guard: StackGuard,
}
impl Sema<'_, '_> {
    // parser actions (069)
    pub fn act_template_head(&mut self, params: &[ParamSyntax], requires: Option<ExprId>, depth: u16) -> TemplateParamListId;
    pub fn act_template_id(&mut self, name: TemplateNameId, args: &[TemplateArg], l: Loc, r: Loc) -> Result<TemplateIdRef, ()>;
    pub fn act_dependent_name(&mut self, nns: NnsId, name: Symbol, targs: Option<ArgListId>, kw: Keyword) -> QualType;
    pub fn act_explicit_instantiation(&mut self, extern_loc: Option<Loc>, d: DeclSyntax) -> Option<DeclId>;
    pub fn act_concept(&mut self, head: TemplateParamListId, name: Symbol, e: ExprId) -> DeclId;
    pub fn act_requires_expr(&mut self, params: &[DeclId], reqs: &[Requirement]) -> ExprId;
    pub fn act_fold(&mut self, lhs: Option<ExprId>, op: BinOp, rhs: Option<ExprId>, ellipsis: Loc) -> ExprId;
    pub fn act_pack_expansion(&mut self, pattern: ExprId, ellipsis: Loc) -> ExprId;
    pub fn act_deduction_guide(&mut self, …) -> DeclId;
    // engine (070–074)
    pub fn check_template_args(&mut self, tmpl: DeclId, written: &[TemplateArg], loc: Loc) -> Result<ConvertedArgs, ()>;
    pub fn get_class_spec(&mut self, tmpl: DeclId, args: &ConvertedArgs, loc: Loc) -> DeclId;
    pub fn get_var_spec(&mut self, tmpl: DeclId, args: &ConvertedArgs, loc: Loc) -> DeclId;
    pub fn get_function_spec(&mut self, tmpl: DeclId, args: ArgListId) -> DeclId;
    pub fn require_complete(&mut self, t: QualType, loc: Loc, diag: Option<DiagId>) -> bool;
    pub fn instantiate_class(&mut self, spec: DeclId, poi: Loc) -> bool;
    pub fn instantiate_function_def(&mut self, f: DeclId, poi: Loc) -> Option<StmtId>;
    pub fn instantiate_var_def(&mut self, v: DeclId, poi: Loc) -> bool;
    pub fn mark_referenced(&mut self, d: DeclId, loc: Loc);
    pub fn deduce_call(&mut self, ft: DeclId, explicit: Option<&[TemplateArg]>, args: &[ExprId], ctx: CallCtx) -> Result<DeclId, DeductionFailure>;
    pub fn deduce_auto(&mut self, declared: QualType, init: ExprId, kind: AutoInit) -> Result<QualType, AutoFail>;
    pub fn more_specialized(&mut self, a: DeclId, b: DeclId, ctx: PoCtx, nargs: u16) -> Option<DeclId>;
    pub fn check_satisfaction(&mut self, entity: DeclId, args: ArgListId, loc: Loc) -> SatId;
    pub fn end_of_tu_instantiations(&mut self);   // drains `pending` in a loop
}
```

**`AstBuilder` constructors.** All of these compute deps per §3.2:
- `template_param_list`, `template_type_parm`, `non_type_template_parm`,
  `template_template_parm`;
- `class_template`, `function_template`, `var_template`,
  `alias_template`, `concept`;
- `class_spec`, `var_spec`, `function_spec` (each with a `SpecInfo`),
  `partial_spec`, `member_of_spec`;
- `subst_nttp_expr`, `pack_expansion_expr`, `fold_expr`, `size_of_pack`,
  `pack_indexing_expr`;
- `requires_expr`, `concept_spec_expr`;
- `unresolved_lookup`, `dependent_scope_decl_ref`, `dependent_member`,
  `unresolved_construct`;
- `param_object`, `deduction_guide`.

Two more are needed:
- `set_definition_once(decl, StmtId | ExprId)`. Instantiated definitions
  fill an empty slot exactly once; this is append-only (ast.md §8.4).
- `set_members_once(record, ListId)`, used by `instantiate_class`.

**`SemaBridge`.**
- `ensure_body(&mut self, f, oven: &mut Oven) -> Option<StmtId>` as in
  §4.11 (R-OVEN-1).
- R-OVEN-2: frozen values are hash-consed.
- A new `EvalKind::Constraint`.

**Landed (WP-069).** Parsing and the parse-time sema for templates are
in place. The engine (§4–§10) is not.

*Files.*
- `kiln_parse`:
  - `src/templ.rs`: heads, parameters, template-ids, the `<` rule,
    requires-clauses and requires-expressions, concepts, and the
    specialization/instantiation syntax;
  - hooks in `src/{tokens,decl,expr,stmt,parser}.rs`;
  - unit tests in `tests/templ.rs`.
- `kiln_sema`:
  - `src/templ/mod.rs`: heads and parameter actions, abbreviated
    templates, re-entering template scopes, and the §2.1 checks;
  - `src/templ/actions.rs`: template-ids, dependent names, packs and
    folds, concepts and requires-expressions, `classify_name` support,
    and dependence queries;
  - hooks in `src/{decl,declspec,expr,class/members,lookup,…}.rs`.
- `kiln_ast`:
  - `src/templates.rs`: the tables and the constructors listed below;
  - `types/print.rs` gains `write_arg_list`;
  - `dump/names.rs` prints dependent expressions as clang's
    `printPretty`;
  - the template dump lines (ast-dump.md §5).
- `kiln_diag`: the template diagnostic ids, appended at the end of the
  tables.
- Tests:
  - `tests/conformance/templates/parse/` holds 46 `expect: ast` goldens.
  - `tests/conformance/templates/diag/` holds 10 strict-diags tests.

*Signatures as landed* (where they differ from the sketch above):
```rust
pub fn act_on_template_params_start(&mut self, template_loc: Loc, l_angle: Loc) -> u16;
pub fn act_on_type_template_param(&mut self, typename_kw: bool, kw_loc: Loc, pack: bool, name: Option<(Symbol, Loc)>) -> DeclId;
pub fn act_on_constrained_template_param(&mut self, concept: DeclId, cargs: &[TemplateArg], loc: Loc, pack: bool, name: Option<(Symbol, Loc)>) -> DeclId;
pub fn act_on_non_type_template_param(&mut self, p: &ParamInfo, pack: bool) -> DeclId;
pub fn act_on_template_template_param(&mut self, params: TemplateParamListId, typename_kw: bool, kw_loc: Loc, pack: bool, name: Option<(Symbol, Loc)>) -> DeclId;
pub fn act_on_template_param_default(&mut self, p: DeclId, arg: TemplateArg, loc: Loc); // loc = the argument, not `=`
pub fn act_template_head(&mut self, requires: Option<ExprId>, r_angle: Loc) -> TemplateParamListId; // params pushed one by one
pub fn act_on_finish_template_decl(&mut self);
pub fn act_on_invented_template_param(&mut self, loc: Loc, pack: bool, constraint: Option<(DeclId, Vec<TemplateArg>)>, param_name: Option<Symbol>) -> QualType;
pub fn act_on_end_abbreviated(&mut self);
pub fn act_on_trailing_requires(&mut self, f: DeclId, r: ExprId);
pub fn act_on_reenter_template_scopes(&mut self, f: DeclId) -> usize;
pub fn match_template_lists_to_scope(&self, qualifier: Option<DeclId>) -> usize;
pub fn act_on_specialization_declarator_id(&mut self, tmpl: DeclId, loc: Loc) -> bool; // false: declarator dropped
pub fn act_template_id(&mut self, name: TemplateNameId, args: &[TemplateArg], l: Loc, r: Loc) -> Result<TemplateIdRef, ()>;
pub fn act_on_template_id_call_name(&mut self, e: ExprId);
pub fn act_dependent_name(&mut self, nns: NnsId, name: Symbol, targs: Option<ArgListId>, kw: TagKeyword) -> QualType;
pub fn act_dependent_scope_ref(&mut self, nns: NnsId, name: Symbol, targs: Option<ArgListId>, loc: Loc) -> ExprId;
pub fn try_dependent_member(&mut self, base: ExprId, name: Symbol, targs: Option<ArgListId>, template_kw: bool, loc: Loc, arrow: bool) -> Option<ExprId>;
pub fn act_on_undeclared_call_name(&mut self, name: Symbol, loc: Loc) -> ExprId; // ADL-deferred ULE
pub fn act_on_nested_name_component(&mut self, prefix: Option<(DeclId, NnsId)>, name: Symbol, loc: Loc) -> Option<(DeclId, NnsId)>;
pub fn act_on_nested_name_template_id(&mut self, prefix: Option<NnsId>, tmpl: DeclId, ty: QualType, template_kw: bool, loc: Loc) -> Option<(DeclId, NnsId)>;
pub fn act_concept(&mut self, head: TemplateParamListId, name: Symbol, loc: Loc, e: ExprId) -> DeclId;
pub fn act_on_requires_expr_start(&mut self, loc: Loc) -> DeclId;
pub fn act_on_requires_param(&mut self, body: DeclId, p: &ParamInfo, idx: u16) -> DeclId;
pub fn act_requires_expr(&mut self, body: DeclId, params: &[DeclId], reqs: &[Requirement], loc: Loc, rbrace: Loc) -> ExprId;
pub fn act_on_return_type_requirement(&mut self, concept: DeclId, args: &[TemplateArg], loc: Loc) -> ExprId;
pub fn act_pack_expansion(&mut self, pattern: ExprId, ellipsis: Loc) -> ExprId;
pub fn act_pack_expansion_type(&mut self, pattern: QualType, ellipsis: Loc) -> QualType;
pub fn act_pack_expansion_arg(&mut self, a: TemplateArg, ellipsis: Loc) -> TemplateArg;
pub fn act_fold(&mut self, lhs: Option<ExprId>, op: BinaryOp, rhs: Option<ExprId>, lparen: Loc, ellipsis: Loc, rparen: Loc) -> ExprId;
pub fn act_on_sizeof_pack_name(&mut self, name: Symbol, name_loc: Loc, loc: Loc, rparen: Loc) -> ExprId;
pub fn act_explicit_instantiation(&mut self, extern_loc: Option<Loc>, loc: Loc) -> Option<DeclId>;  // stub
pub fn act_explicit_specialization(&mut self, partial: bool, loc: Loc) -> Option<DeclId>;           // stub
pub fn act_deduction_guide(&mut self, name: Symbol, loc: Loc) -> Option<DeclId>;                    // stub
pub fn templ_unimplemented(&self, feature: &str, loc: Loc); // sema_unimplemented_template_feature
```

The `AstBuilder` constructors (`kiln_ast/src/templates.rs`) are:
- `template_param_list`, `template_type_parm`, `non_type_template_parm`
  and `template_template_parm`;
- `template_decl` (class, function, variable and alias) and `concept`;
- `template_arg_expr`, `dependent_scope_decl_ref`, `unresolved_lookup`,
  `dependent_member` and `unresolved_construct`;
- `dependent_unary`, `dependent_binary`, `dependent_conditional` and
  `paren_list`;
- `size_of_pack`, `pack_expansion_expr` and `fold_expr`;
- `requires_expr` and `concept_spec_expr`.

The dependent-reference constructors (`dependent_scope_decl_ref`,
`unresolved_lookup`, `dependent_member`) also inherit
`UNEXPANDED_PACK` and `CONTAINS_ERROR` from their nested-name-specifier.

*Interface changes outside the templates crates.*
- `kiln_ast`:
  - `types::write_arg_list`;
  - `AstNames::expr` prints DSDRE, ULE and DSME as written;
  - dependent `decltype` is canonical.
- `kiln_sema`:
  - `Declarator.dropped`: a dropped specialization declarator declares
    nothing.
  - `classify_name` maps an injected-class-name used with `<` to
    `TypeTemplate`.
  - `injected_type_of` no longer marks a declaration referenced.
  - `set_adl` and `decay_dependent`.
  - `dependent_subscript_type` and the dependent `ToVoid` cast.
  - Dependent expressions get no unused-result warnings.
  - `check_template_param_list` and `check_template_param_shadow`
    implement [temp.param]/14 and [temp.local]/6.
  - `default_locs`.
  - [depr.static.constexpr]: an out-of-line `constexpr const T X::v;`
    without an initializer is a redundant redeclaration, not a
    redefinition (`class/members.rs`, `decl.rs::act_on_uninit`).
- `kiln_parse`:
  - `parse_nested_name_nns` and `entering_context`;
  - `diagnose_missing_template_kw`, which reports
    `parse_template_kw_missing`;
  - `parse_specialization_declarator_id`;
  - an alias-declaration may carry `__attribute__` after its name;
  - `at_partial_specialization` skips `[[…]]`, `__attribute__((…))` and
    `alignas(…)`.

*Gaps* (each reports a diagnostic or is a known divergence):
- The shadow check does not cover function parameters. Kiln emits a
  plain error where clang emits the ExtWarn
  `-Wstrict-primary-template-shadow`.
- Inherited defaults are not recorded on the redeclaration's
  parameters. A class-template forward redeclaration creates no new
  decl, so chained default checks are partial.
- A template template parameter's inner list is not order-checked.
- P0846 (ADL for an unknown name followed by `<`) is not implemented.
- A `DependentScopeDeclRefExpr` does not record the `template` keyword.
- A parameter named in its own declarator's trailing return type is
  unsupported.

*Stubs for WP-070a/b and later* (each reports
`sema_unimplemented_template_feature` or `sema_unimplemented`):
- A non-dependent `class template specialization` (needs
  `get_class_spec` and instantiation).
- Explicit and partial specializations of class, variable and function
  templates (`act_explicit_specialization`,
  `act_on_specialization_declarator_id`).
- Explicit instantiation (`act_explicit_instantiation`).
- Deduction guides (`act_deduction_guide`).
- Alias-template substitution.
- Calls to function templates (deduction, WP-071).

*`<type_traits>` probe*: run
`kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0` on
`#include <type_traits>`. The error count went from 464 on main to 431.
No remaining error is a WP-069 template-syntax failure. The blockers
are:

| Errors | Cause | Owner |
|---:|---|---|
| ~207 | Builtin traits and transforms (`__is_same`, `__remove_cvref`, …) | WP-074 |
| 74 | Partial and explicit specializations | WP-070b |
| 23 | Non-dependent specializations | WP-070a |
| ~19 | Operators, constructors and conversion functions | WP-067a/068a |

Most of the remaining errors are follow-ons of these.

**Landed (WP-070a).** Class and alias templates are instantiated:
argument checking and conversion, the substituter, class
specializations and their members, the instantiation stack and its
notes, and the report. Partial and explicit specialization, deduction,
packs, satisfaction and the builtins are stubs (below).

*Files.*
- `kiln_sema/src/templ/`:
  - `args.rs`: `ConvertedArgs`, `MultiLevelArgs` (`Level`, `LevelMode`,
    `ArgLookup`), `check_template_args`, per-kind checks, NTTP
    conversion, defaults and canonical/printing argument lists;
  - `class.rs`: `get_class_spec`, `class_template_type`,
    `alias_template_type`, `require_complete*`, `instantiate_class`,
    lazy member-function, static-data-member and member-class
    instantiation;
  - `stack.rs`: `FrameKind`, `InstFrame`, `InstLimits`,
    `InstantiationStack`, `SfinaeState`, `push_inst_frame` /
    `pop_inst_frame` and the §10.1 notes;
  - `report.rs`: `InstReport`, `InstRow`, `RowKind`, `ReportFormat` and
    the text/JSON renderers (§11.3);
  - `pending.rs`: the end-of-TU queue; `deduce.rs` and `order.rs`:
    stubs;
  - `subst/{mod,types,exprs,stmts,decls}.rs`: `Substituter`.
- `kiln_ast/src/templates.rs`: `SpecInfo`, `MemberSpecInfo`,
  `SpecKind`, the specialization and member tables, `expr_begin`, and
  `class_spec`, `member_of_spec` and `subst_nttp_expr`.
- `kiln_ast/src/dump`: the `ClassTemplateSpecializationDecl` and
  `SubstNonTypeTemplateParmExpr` lines (ast-dump.md §5), mirrored in
  `xtask/src/clang_ast`.
- `kiln_driver/src/templ_opts.rs`: the template flags.
- `xtask/src/test/inst_report.rs`: the `inst-report:` directive.
- `bench/frontend/templates_{wide,deep}.cpp` with kiln baselines.
- Tests: `tests/conformance/templates/` gains 29 `class/`, 17 `alias/`,
  19 `diag/` (strict-diags, clang 19 texts) and 1 `report/` test.

*Signatures as landed* (where they differ from §11.2):
```rust
pub fn check_template_args(&mut self, tmpl: DeclId, written: &[TemplateArg], arg_locs: &[Loc], loc: Loc) -> Result<ConvertedArgs, ()>;
pub fn get_class_spec(&mut self, tmpl: DeclId, written: &[TemplateArg], arg_locs: &[Loc], loc: Loc) -> Result<DeclId, ()>;
pub fn class_template_type(&mut self, name: TemplateNameId, tmpl: DeclId, written: &[TemplateArg], arg_locs: &[Loc], loc: Loc) -> Result<QualType, ()>;
pub fn alias_template_type(&mut self, name: TemplateNameId, tmpl: DeclId, written: &[TemplateArg], arg_locs: &[Loc], loc: Loc) -> Result<QualType, ()>;
pub fn require_complete(&mut self, q: QualType, loc: Loc) -> bool;        // diagnoses
pub fn require_complete_silent(&mut self, q: QualType, loc: Loc) -> bool; // instantiates, no diagnostic
pub fn require_complete_nns(&mut self, q: QualType, loc: Loc) -> bool;    // [expr.prim.id.qual] texts
pub fn instantiate_class(&mut self, spec: DeclId, poi: Loc, diagnose: bool) -> bool;
pub fn instantiate_function_definition(&mut self, f: DeclId, poi: Loc);
pub fn instantiate_static_member_init(&mut self, v: DeclId, poi: Loc);
pub fn end_of_tu_instantiations(&mut self);
pub(crate) fn push_inst_frame(&mut self, kind: FrameKind, entity: DeclId, template: DeclId, poi: Loc) -> bool; // false: depth limit or fatal
pub(crate) fn pop_inst_frame(&mut self);
pub fn instantiation_report(&self, file: &str) -> String; // "" unless -fkiln-instantiation-report
pub fn act_dependent_scope_ref(&mut self, nns: NnsId, name: Symbol, targs: Option<ArgListId>, begin: Loc, loc: Loc) -> ExprId;
impl Substituter { pub fn new(sema: &mut Sema, args: &MultiLevelArgs) -> Self; pub fn map_context(&mut self, pattern: DeclId, inst: DeclId); … }
pub fn deduce_call(&mut self, tmpl: DeclId, explicit: Option<&ConvertedArgs>, args: &[ExprId], loc: Loc) -> Option<ConvertedArgs>; // stub
```
SFINAE is `InstantiationStack::is_sfinae()` plus a `SfinaeState`
stack in `InstState.sfinae`: `Sema::emit` counts errors and keeps the
first instead of emitting them (§5.7).

*Interface changes outside the templates crates.*
- `kiln_sema`:
  - `Sema.inst: InstState`; `Sema::emit` prepends the instantiation
    notes and captures errors under SFINAE.
    `InstState.suppress_next_error` drops one follow-on error, and
    `InstState.undefined_diagnosed` reports an undefined template once.
  - `act_dependent_scope_ref` gains `begin` (the qualifier's first
    token, kept in `ast.templates.expr_begin`).
  - Member access into an incomplete class type is diagnosed at the
    operator as `member access into incomplete type {0}`
    (`sema_member_reference_incomplete`).
  - A dependent enum-base and a dependent `constexpr` variable without
    an initializer are checked at instantiation.
- `kiln_ast`: `Ast::node_live_bytes` and `node_count` (the meter),
  `ExprArena::subst_replacement` / `subst_parameter`, `VarSpec::LATE`.
- `kiln_parse`: `ParseOptions.inst_limits` and `inst_report`, and
  `ParsedTu.inst_report` (the rendered report).
- `kiln_pp`: `Preprocessor::main_file`.
- `kiln_oven`: looks through `SubstNonTypeTemplateParmExpr`.
- `kiln_driver`:
  - `-ftemplate-depth=N` (default 1024), `-ftemplate-backtrace-limit=N`
    (default 10, 0 = all) and `-fkiln-instantiation-report[=all|json|text]`
    (the report goes to stderr; timing only with the flag);
    `ArgError::InvalidFlagValue` maps to `drv_invalid_value`;
  - `main` runs the compile on a thread with a 1 GiB reserved stack.
    *WP-086:* this is now the Q4 sema thread, sized by
    `-fkiln-stack-size` (default 1 GiB) with `StackGuard` installed
    (§10.3).
- `kiln_diag`: the instantiation notes, `sema_template_recursion_depth`
  (fatal), the undefined-template, arity, argument-kind, nested-name and
  `typename` errors, appended at the end of the tables.
- Harness: the `inst-report:` directive (harness.md) with masked
  timings; the clang-ast mapping keeps `ClassTemplateSpecializationDecl`
  and `SubstNonTypeTemplateParmExpr`.

*Gaps* (each reports a diagnostic or is a known divergence):
- No TypeLocs: an error inside a written type points at the
  declarator, not the type.
- Default member initializers are instantiated with the class. clang
  instantiates them when used, with its own notes.
- ~~A qualified `DeclRefExpr` prints without its qualifier~~ *Resolved
  (WP-086):* `act_on_id_expression` records the nested-name-specifier in
  `DeclRefExtra.qualifier` and the qualifier's first token as the begin
  location (`ast.templates.expr_begin`); the printer writes it before the
  name, as clang 19's StmtPrinter does (`'N::x == 2'`, `'::z == 3'`,
  `'A<int>::y == 4'`), and substitution substitutes it. clang's dumps
  show no qualifier, so the AST goldens are unchanged. Still missing:
  implicit member references named with a qualifier (`class/member_expr.rs`).
- No `CXXConstructExpr` for a class-typed variable of a specialization
  (WP-067b).
- ~~Parsing nested template-ids is quadratic~~ *Resolved (WP-086):*
  `skip_template_args` results are memoized per token (parse.md §2.5);
  `templates_deep.cpp` dropped from 406 ms to ~60 ms in release.
- ~~The `inst.*` bench metrics are not in `xtask bench`~~ *Resolved
  (WP-086):* see §11.4.
- Known divergences: a `void` parameter list after substitution, the
  value-initialization temporary in an instantiated member-function
  body, friend redefinition across specializations, and
  explicit-after-implicit mismatches.

*Stubs* (each reports `sema_unimplemented_template_feature`):
| Stub | Owner |
|---|---|
| Explicit specialization (`act_explicit_specialization`), explicit instantiation | WP-070b (done) |
| Partial-specialization matching (`match_partial_specs`), `more_specialized` | WP-070b (done for classes and variables) / WP-071 |
| Member templates; the dependent template specialization type (`T::template X<…>`) | WP-070b (done) |
| `deduce_call`, function template specializations | WP-071 |
| Pack expansion (`expand_list` with an expansion), variadic specializations | WP-072 |
| `check_satisfaction` | WP-073 |
| The builtins `TABLE` | WP-074 |
| Pointer-to-member template arguments | WP-071 |

*`<type_traits>` probe* (same command as WP-069): 431 errors on main
before WP-070a, then 415 (the main binary at merge time gives 426). The
23 non-dependent specialization errors are gone. The blockers are:

| Errors | Cause | Owner |
|---:|---|---|
| ~200 | Builtin traits and transforms (`__is_same`, `__remove_cvref`, …) | WP-074 |
| 87 | Partial (60) and explicit (27) specializations | WP-070b / WP-072 |
| ~19 | Operators, constructors and conversion functions | WP-067b/068 |
| 3 | `__copy_cv<_From>::template __apply<_To>` | WP-070b |

Q3 was not implemented; see §14.

**Landed (WP-074).** The builtins of §9, `auto`/`decltype(auto)`
deduction for variables and return types, `decltype` (including the
id-expression and call rules), and the staging of `__has_builtin`.
Overload-resolution traits and pack arguments are stubs (below).

*Files.*
- `kiln_sema/src/templ/builtins/`:
  - `table.rs`: `TABLE` (the §9.3–9.5 rows), the per-trait `arity`,
    `completeness`, `deprecated_replacement`, `trait_status` and
    `trait_kind`, `lookup`, `transform_by_name`,
    `has_builtin_staged` / `has_constexpr_builtin_staged`;
  - `traits.rs`: `act_type_trait`, `build_type_trait`,
    `build_array_type_trait`, `build_expression_trait` and the §9.1
    predicates (`is_trivially_copyable_type`, `dtor_nothrow`, …);
  - `transforms.rs`: `build_unary_transform` (§9.2) and `referenceable`;
  - `builtin_templates.rs`: `declare_builtin_templates` and
    `builtin_template_id` (`__make_integer_seq`, `__type_pack_element`).
- `kiln_sema/src/templ/auto.rs`: `deduce_auto`, `AutoInit`, `AutoFail`,
  `AutoGroup`, `act_on_init_in_group`, `check_auto_group`, `placeholder_keyword`, `dependent_auto`,
  `replace_placeholder`, `std_initializer_list`.
- `kiln_sema/src/deduce.rs`: the `decltype` scope
  (`enter_decltype` / `exit_decltype` and the deferred call checks),
  `act_on_decltype`, `decltype_of`, `deduce_placeholder`, `deduce_from`,
  `deduce_return`, `set_deduced_return`, `check_undeduced_use`.
- `kiln_sema/src/builtins.rs`: `__builtin_addressof`,
  `act_on_builtin_bit_cast` (and the §9.5 functions, WP-088 below).
- `kiln_ast`: `TypeTraitExpr`, `ArrayTypeTraitExpr` and
  `ExpressionTraitExpr` (`ExprArena::type_trait_*`, `array_trait`), the
  `UnaryTransform` type print, `BuiltinTemplate` decls,
  `CastKind::LValueToRValueBitCast`, `builtin::ADDRESSOF`; dump lines
  for each, and `AstNames::canonical_expr_types`, so that
  `static_assert` messages print canonical types the way clang does.
- `kiln_parse`: the trait keywords (`keywords.rs`), the trait,
  transform and `__builtin_bit_cast` grammar (`expr.rs`,
  `tentative.rs`), the `decltype` scope, a decltype-specifier as a
  base-specifier (`class.rs`; `decltype(auto)` there is
  `parse_decltype_auto_invalid`), and the multi-declarator `auto` check
  in `parse_simple_declaration`.
- Tests:
  - `tests/conformance/templates/builtins/` (34): a positive and a
    negative test per §9 row, plus a clang probe golden;
  - `tests/conformance/templates/auto/` (33);
  - `tests/conformance/dcl.type.decltype/` (9);
  - `tests/conformance/dcl.spec.auto/undeduced-member-diag.cpp`.

  Each test names its section and passes under
  `--compiler clang++-19`.

*Signatures as landed.*
```rust
pub fn deduce_auto(&mut self, declared: QualType, init: &[Option<ExprId>], kind: AutoInit) -> Result<QualType, AutoFail>;
pub fn act_on_init_in_group(&mut self, group: &mut AutoGroup, v: DeclId, init: Initializer, at: Loc); // act_on_init + [dcl.spec.auto.general]/8
pub fn enter_decltype(&mut self);
pub fn exit_decltype(&mut self, e: Option<ExprId>);
pub fn act_on_decltype(&mut self, e: ExprId, loc: Loc, rparen: Loc) -> Option<QualType>;
pub fn act_type_trait(..) / build_type_trait(..) / build_array_type_trait(..) / build_expression_trait(..) -> ExprId;
pub fn build_unary_transform(..) -> QualType;
pub fn act_on_builtin_bit_cast(..) -> ExprId;
pub fn has_builtin_staged(name: &str) -> Option<bool>;           // None: not a table name
pub fn has_constexpr_builtin_staged(name: &str) -> Option<bool>;
```

*Behaviour.*
- The placeholder seam with WP-070b is
  `Auto { deduced: Some(QualType::DEPENDENT) }`. A type-dependent
  initializer or return operand gives a dependent placeholder.
  Substitution turns it back into an undeduced one, and the
  instantiation then deduces it again. `templ/deduce.rs` is not
  called: `deduce_auto` does [temp.deduct.call] for one P/A pair. The
  pair forms are `auto`, `auto*`, `auto&`, `auto&&`, cv, and
  `std::initializer_list<auto>`. WP-070b/071 can replace that core
  with `deduce` without changing the callers (§6.3).
- `decltype(e)` follows [dcl.type.decltype]/1. An unparenthesized
  id-expression or member access gives the declared type. Otherwise
  the result follows the value category: xvalue → `T&&`,
  lvalue → `T&`, prvalue → `T`. A prvalue call operand whose return
  type is incomplete is accepted only when the call is the whole
  operand ([dcl.type.decltype]/2). Other calls are checked when the
  scope exits.
- Calling a function whose return type is still undeduced instantiates
  it if it is a template specialization member (`check_undeduced_use`).
  Otherwise it reports
  `function {0} with deduced return type cannot be used before it is defined`.
- `__has_builtin` (and `__has_constexpr_builtin`) consult the table
  only in the sema modes (`-fsyntax-only`, `-emit-llvm`, …;
  `build_preprocessor(.., staged: true)`). `-E` and `-M` answer as
  clang does, so their output stays token-identical.
  A `Status::Stub` row answers false, so libc++ uses its library
  fallback. Using a stub row directly reports
  `sema_unimplemented_template_feature`.

*Interface changes outside the templates crates.*
- `kiln_sema`:
  - `Sema.decltype_calls`;
  - `build_decl_ref_via` diagnoses a variable used in its own `auto`
    initializer, and calls `check_undeduced_use` for methods as well as
    functions;
  - `act_on_field` rejects `auto` non-static members;
  - virtual functions with a deduced return type are rejected;
  - a differing return type on a redeclaration notes "previous
    definition" when the earlier declaration has a body;
  - `class/mod.rs` re-exports `special::{Selected, SpecialKind}` for the
    traits (a WP-067b merge touchpoint);
  - `conv::eval_width` is now `pub(crate)`.
- `kiln_oven` and `kiln_codegen` fold the trait expressions to
  constants, and handle `LValueToRValueBitCast` (scalar only) and
  `__builtin_addressof`.
- `kiln_driver`: `Job::build_preprocessor` gains `staged: bool`.
- `kiln_diag`: the §9 and `auto` errors, appended at the end:
  - `sema_type_trait_arity`, `sema_incomplete_type_trait`,
    `sema_deprecated_builtin` (group `-Wdeprecated-builtins`);
  - `sema_dimension_expr_not_const`, `sema_make_signed_bad_type`,
    `sema_only_enums_have_underlying`, `sema_pack_index_out_of_bounds`;
  - `sema_integer_seq_*`, `sema_bit_cast_*`;
  - `sema_auto_var_*`, `sema_auto_inconsistent_deduction`,
    `sema_decltype_auto_*`, `sema_auto_not_allowed_field`,
    `sema_auto_fn_virtual`;
  - `sema_call_incomplete_return`,
    `sema_call_function_incomplete_return`, `sema_cast_to_incomplete`,
    `sema_incomplete_type_required`;
  - `parse_decltype_auto_invalid`.

*Stubs* (each reports `sema_unimplemented_template_feature`, and
`__has_builtin` answers false):
| Stub | Owner |
|---|---|
| The 17 overload-resolution traits (`__is_constructible`, `__is_assignable`, `__is_convertible`, `__is_nothrow_*`, `__reference_*_from_temporary`, …) | WP-068a |
| `__is_layout_compatible`, `__is_pointer_interconvertible_base_of`, `__can_pass_in_regs`, `__is_bitwise_cloneable` | M4 (unassigned) |
| `__is_trivially_relocatable` (`TRIVIALLY_RELOCATABLE` is never set) | unowned: S3C canPassInRegisters |
| `__type_pack_element` and `__make_integer_seq` with pack arguments | WP-072 |
| `__builtin_constant_p` (sema and Oven) | unowned: not in WP-088 |
| `__builtin_operator_new` / `__builtin_operator_delete` in the Oven (sema and codegen landed in WP-088) | WP-061b |

The other §9.5 functions, the source-location builtins and
`__builtin_bit_cast` in the Oven landed in WP-088 (§9.5). In all, 157
table names answer yes and 25 are staged off.

*Gaps* (each reports a diagnostic or is a known divergence):
- Substituted types carry no argument locations, so the columns of
  some trait errors inside instantiations differ from clang.
- An array of an incomplete type is rejected when the declarator is
  formed, before a trait can see it.
- Abominable function types (`void() const`) are not diagnosed as
  trait operands.
- `ExcSpec::Deferred` is not resolved. `dtor_nothrow` computes the
  implicit destructor's exception specification itself.
- `x.f()` with `f` an undeduced member goes through
  `class/member_expr.rs` (WP-067b). That path has no
  `check_undeduced_use` and does not instantiate on use; its return
  type should come from `call_result_of`.
- Inline member functions defined in the class are not emitted by
  codegen, so the `run` tests use namespace-scope functions.
- `auto x = {1, 2}` deduces `std::initializer_list<int>`. The
  initialization of the `initializer_list` object belongs to WP-068a.
- A failed member typedef in an instantiation is dropped, which causes
  follow-on errors. A `constexpr` static member without an initializer
  reports twice.

*`<type_traits>` probe* (same command as WP-069): 410 errors before
WP-074, 148 after. None of the remaining errors is a §9 builtin:

| Errors | Cause | Owner |
|---:|---|---|
| 64 + 2 | Partial specializations (class, variable) | WP-070b / WP-072 |
| 27 | Explicit specializations | WP-070b |
| 17 | Parameters named in a trailing return type | fixed by WP-087 (131 after) |
| 20 | Overloaded operators and overloading | WP-068 |
| 4 | Declarations that differ only in return type (overloading) | WP-068 |
| 7 + 1 | `common_reference.h`: `typename` on an alias template-id, and the `?:` operand types | WP-070b / WP-068 |
| 4 | Dependent template specialization type | WP-070b |
| 2 | "this kind of name" | WP-070b |

**Landed (WP-087).** The `auto`/`decltype` follow-ups to WP-074; the
four gaps above that they close are removed.

- *Parameters in their declarator* ([basic.scope.param],
  [dcl.fct.default]/9, [expr.prim.this]/2): sema.md §4.3. Instantiated
  member signatures substitute the parameters before the return type,
  so `-> decltype(x)` sees the substituted parameter.
- *Deduced-return redeclarations* ([dcl.spec.auto.general]/13):
  redeclarations compare the declared placeholder type and adopt the
  deduced one (sema.md §4.3.1); `set_deduced_return` no longer makes a
  later `auto f();` an error.
- *One deduction per declarator group* ([dcl.spec.auto.general]/3, /8):
  member groups, `static constexpr auto x = …, y = …;` and local groups
  re-checked at instantiation, plus clang's "function with deduced
  return type must be the only declaration in its group" (sema.md
  §4.3.1).
- *`-Wunevaluated-expression`* ([expr.context]/1), in `-Wmost` (sema.md
  §4.3.1).
- *Deduplication per instantiation*: §10.2 and spec/diag.md §7.4.

*Interface changes.*
- `kiln_ast`: `AstBuilder::prototype_param`,
  `DeclArena::set_param_owner`.
- `kiln_sema`: `ParamInfo.decl`; `act_on_prototype_param` takes
  `&mut ParamInfo`; `act_on_start_this_scope` /
  `act_on_end_this_scope`; `check_default_arg_refs`;
  `AutoGroup { decls }` with `act_on_group_declarator` and
  `act_on_end_group` (replacing `act_on_init_in_group`; callers use
  plain `act_on_init`); `Sema` fields `prototype_params` (now
  `PrototypeFrame`s of `ParmVar` ids), `unowned_params`,
  `declared_returns`, `group_placeholders`;
  `InstantiationStack::context_key`.
- `kiln_diag`: `Diag::context(u64)`, `DiagEngine::would_emit`; the
  dedup key is `(id, span, context)`; ids `sema_auto_non_deduced_not_alone`
  and `sema_side_effects_unevaluated_context`, group
  `-Wunevaluated-expression`.
- Touchpoints in other packages' files (one or a few lines each):
  `templ/class.rs` (`SavedState` keeps `prototype_params`),
  `templ/subst/decls.rs` (parameters before the return type),
  `templ/subst/stmts.rs` (the group re-check), `kiln_parse/src/class.rs`
  (member groups; the replayed default argument's check).

*Tests:* `dcl.fct/trailing-return-params.cpp`,
`dcl.fct/default-arg-references-param.cpp`,
`dcl.spec.auto/redeclaration-chain.cpp`,
`dcl.spec.auto/group-members.cpp`,
`expr.sizeof/unevaluated-side-effects.cpp`,
`templates/diag/repeat-across-instantiations.cpp`;
`basic.scope.param/param-in-declarator.cpp` is no longer xfail;
`kiln_diag/tests/engine.rs` `dedup_context`. Each passes under
`--compiler clang++-19`.

*Remaining gaps.*
- An out-of-line member declarator (`auto X::f() -> T`) does not enter
  the class scope for its trailing return type.
- `this` is accepted in the trailing return type of a static member
  function or a friend.
- Side effects inside `CXXDefaultArgExpr`, statement expressions and
  lambda init-captures are not inspected; an operand that is only
  instantiation-dependent may warn where clang does not; clang's
  `warn_side_effects_typeid` (a potentially evaluated `typeid` operand)
  is not implemented.
- Braced default arguments are not checked by
  `check_default_arg_refs`.
- The parameters of a nested function declarator (a function-pointer
  parameter's own parameters) and of a destructor declarator are never
  adopted: they keep their provisional context (`unowned_params`).
- `decltype(param)` in a dependent return type profiles the
  `DeclRefExpr` by declaration, not by parameter index as clang does,
  so two redeclarations of a function template with such a return type
  may not match.
- A non-dependent `static_assert` in a template definition is evaluated
  in the definition; clang (CWG2518) waits for an instantiation, and
  then reports it once per instantiation.
**Landed (WP-070b).** The deduction core, class and variable template
partial specializations with their matching and ordering, explicit
specializations and instantiations of class and variable templates,
variable templates, out-of-line static data member definitions, member
templates, `T::template X<…>`, and `ensure_body` instantiation
(R-OVEN-1, §4.11).

*Files.*
- `kiln_sema/src/templ/`:
  - `deduce.rs`: `TdfFlags`, `Tdk`, `DeducedArgs`, `deduce` (types,
    [temp.deduct.type]), `deduce_args` (argument lists), the §10.4
    `deduction_failure_note`. Template template parameters are matched
    through `TemplateName::CanonTemplateTemplateParm`.
  - `order.rs`: `match_partial_specs` / `match_partial_args`
    ([temp.spec.partial.match]) and the partial ordering of class and
    variable partial specializations ([temp.spec.partial.order],
    memoized in `templ.partial_order`). Function template
    `more_specialized` is a stub.
  - `spec.rs` (new): `template<>` and `[extern] template` scopes,
    `act_on_class_template_id_tag` (partial and explicit specialization,
    explicit instantiation, or a reference), partial-specialization
    checks (scope, arguments matching the primary, non-deducible
    parameters, not more specialized, default arguments), explicit
    instantiation of classes and members.
  - `var.rs` (new): `get_var_spec`, variable partial and explicit
    specializations, `instantiate_var_spec_init`.
  - `member.rs` (new): member templates (class, alias and variable) of
    a class specialization, renumbered to the remaining depth.
  - `class.rs`: pattern selection through partial specializations,
    explicit-specialization awareness, static data member out-of-line
    definitions (`instantiate_static_member_def`).
  - `subst/types.rs`, `subst/exprs.rs`: `T::template X<…>` types and
    expressions through `member_template_named`; a non-template after
    `template` is diagnosed.
- `kiln_sema/src/oven_bridge.rs`: `ensure_body`, `with_oven`,
  `instantiate_for_oven`.
- `kiln_ast/src/templates.rs`: `PartialSpecInfo`, `ArgShape` (a
  cheap pre-filter before deduction), the partial tables
  (`partials_of`, `find_partial`, `partial_spec_info`),
  `class_partial_spec`, `var_spec`, `var_partial_spec`,
  `list_implicit_spec`, `declare_spec_in`, `unresolved_lookup_in`.
- `kiln_ast/src/dump` and `xtask/src/clang_ast`: the specialization
  lines (ast-dump.md §5).
- `kiln_parse`: `template<>` and explicit-instantiation heads, the
  template-id tag path (`tag_use`), qualified template-id elaborated
  sugar, and the variable-template declarator path.
- Tests:
  - `tests/conformance/templates/` gains 18 `spec/`, 16 `var/`,
    7 `class/`, 14 `diag/`, 6 `ast/` and 1 `report/` test;
  - `crates/kiln_sema/tests/templ_deduce.rs` (12 unit tests) and
    `crates/kiln_parse/tests/ensure_body.rs` (5).

*Signatures as landed.*
```rust
pub fn deduce(&mut self, p: QualType, a: QualType, flags: TdfFlags, d: &mut DeducedArgs) -> Tdk;
pub fn deduce_args(&mut self, ps: &[TemplateArg], as_: &[TemplateArg], must_match: bool, d: &mut DeducedArgs, loc: Loc) -> Tdk;
pub fn deduction_failure_note(&self, …) -> Option<Diag>;
pub(crate) fn match_partial_specs(&mut self, tmpl: DeclId, spec: DeclId, poi: Loc) -> Result<Option<PartialMatch>, ()>;
pub fn get_var_spec(&mut self, tmpl: DeclId, written: &[TemplateArg], arg_locs: &[Loc], loc: Loc) -> Result<DeclId, ()>;
pub fn act_explicit_spec_start(&mut self, loc: Loc); pub fn act_explicit_spec_end(&mut self);
pub fn act_explicit_inst_start(&mut self, extern_loc: Option<Loc>, loc: Loc); pub fn act_explicit_inst_end(&mut self);
pub fn act_on_class_template_id_tag(&mut self, keyword: TagKeyword, tmpl: DeclId, args: &[TemplateArg], qualifier: Option<DeclId>, use_: TagUse, name_loc: Loc, r_angle: Loc, kw_loc: Loc) -> Result<(Option<DeclId>, QualType), ()>;
pub fn instantiate_static_member_def(&mut self, v: DeclId, poi: Loc);
fn ensure_body(&mut self, f: DeclId, oven: &mut Oven) -> Option<StmtId>; // SemaBridge
```

*Interface changes outside the files WP-070b owns.*
- `kiln_sema`:
  - `Sema.oven: Option<Oven>`; the `Bridge` holds `&mut Sema`, and
    `with_oven` lends the Oven (sema.md §8.1). `InstState.eval_loc`
    holds the location of a manifestly constant-evaluated evaluation.
  - A constexpr member marked while its pattern is undefined is queued
    (`InstState.queued`) and instantiated at the end of the TU.
  - `expr.rs`: a variable template named without arguments is
    diagnosed ("use of variable template 'v' requires template
    arguments", as clang 19).
  - `decl.rs`: a nested-name-specifier through an alias of an
    uninstantiated specialization instantiates it; out-of-line static
    data member definitions of class templates.
  - `templ/class.rs`: `inst_template`, `base_levels`,
    `inst_context_pairs` and `map_contexts` for member templates;
    `member_class_decl` and `undefined_spec` are `pub(crate)`.
  - The `type_parm` signature (it takes the depth).
- `kiln_ast`:
  - `append_member` accepts a `Var` specialization;
  - `TypeContext::mark_dependent_template` / `dependent_templates`;
  - `MemRow::of_set` in the memory meter.
- `kiln_diag`: the deduction notes, the partial-specialization,
  explicit-specialization and explicit-instantiation errors,
  `sema_template_kw_refers_to_non_template`, `note_declared_non_template`
  and `sema_default_arg_in_partial_spec`, plus the groups
  `invalid-partial-specialization`, `unusable-partial-specialization`,
  `instantiation-after-specialization`, `deprecated-redundant-constexpr-static-def`
  and `explicit-specialization-storage-class`. All are appended at the
  end of the tables.

*Gaps* (each reports a diagnostic or is a known divergence):
- Explicit specializations of *members* of an implicit specialization
  (`template<> int A<long>::f();`) are not implemented. They need the
  declarator paths in `decl.rs` and `class/members.rs` (WP-067b's
  area). The test `templates/spec/temp.expl.spec-member.cpp` is xfail.
- Accept-invalid: a member redefinition of a specialization without
  `template<>` is accepted. clang reports "template specialization
  requires 'template<>'".
- No TypeLocs: an error in `typename T::template type<int>` points at
  `typename`, not at the template-id.
- Partial specializations of member templates and out-of-line
  definitions of member templates are not implemented.
- Member template calls (`t.template f<int>()`) wait on WP-071.
- Default arguments of variable partial specializations are not
  checked; clang does not check them either.
- clang wraps some instantiated initializers in `ConstantExpr`; kiln
  does not (no golden depends on it).

*Stubs* (each reports `sema_unimplemented_template_feature`):
| Stub | Owner |
|---|---|
| `deduce_call`, function template specializations, explicit specialization and instantiation of function templates | WP-071 |
| `more_specialized` for function templates ([temp.func.order]) | WP-071 / WP-072 |
| Packs in deduction and in partial specializations (`has_pack`), variadic specializations, pack expansion | WP-072 |
| Constrained partial specializations (satisfaction during matching, ordering by constraints) | WP-073 |
| Pointer-to-member template arguments | WP-071 |
| Member explicit specialization | WP-070c or WP-067b follow-up |

In the WP-070a stub table above, the rows for explicit specialization
and instantiation, partial-specialization matching, member templates
and `T::template X<…>` are done.

*`<type_traits>` probe* (same command as WP-069): 343 errors, down from
415 (426 with the main binary) at the WP-070a merge. No partial or
explicit specialization errors remain. The blockers are:

| Errors | Cause | Owner |
|---:|---|---|
| ~157 | Builtin traits in expressions ("unexpected type name: expected expression") | WP-074 |
| ~57 | Builtin transforms (`__remove_cv_t`, `__decay_t`, `__remove_cvref_t`, …) and their follow-on "expected a type" | WP-074 |
| 30 | Undeclared identifiers (follow-ons, `__int128_t`) | mixed |
| ~15 | Overloaded operators and function overloading | WP-068 |
| — | Function templates (`declval<_Tp>()`, `forward`, `__invokable`) | WP-071 |
| 4 | Constrained class template partial specializations | WP-073 |
**Landed (WP-088): the §9.5 builtin functions.**
- `kiln_ast::decl::builtin`: one `pub const` id per §9.5 function, the
  `attr` bits (`NOTHROW`, `CONST`, `PURE`, `NORETURN`, `CUSTOM`) and
  `Info { id, name, sig, .. }` with clang's `Builtins.td` signature; sema
  declares each from it on first lookup (`Sema::is_builtin_name`).
- `kiln_sema/src/builtins.rs`: custom type-checking for the † rows
  (`builtin_custom_call`: `clzg`/`ctzg`/`popcountg`, the classification
  and ordered-compare families, `launder`, `operator_new`/`_delete`
  selecting the usual replaceable functions), `act_on_source_loc_expr`,
  `note_default_arg_use` (called from `convert_call_args` for every
  defaulted argument) and the `std::source_location::__impl` check.
  `Sema::fold_lone_invalid` keeps clang's single "subexpression not
  valid" note for `__builtin_unreachable`.
- `kiln_ast`: `ExprKind::SourceLocExpr`, `SourceLocKind`,
  `SourceLocValues` and `exprs.cold.source_loc` (spec/ast.md §5.8);
  `kiln_parse` parses the six keywords.
- Diagnostics (`kiln_diag::ids`): `sema_call_too_many_args_at_most`,
  `sema_builtin_invalid_arg_type`, `sema_builtin_fp_class_arg`,
  `sema_builtin_ordered_compare`, `sema_builtin_launder_invalid_arg`,
  `sema_builtin_new_delete_not_usual`,
  `note_builtin_non_usual_declared_here`,
  `sema_std_source_location_impl_not_found`/`_malformed`, all with
  clang's texts.
- Tests: `tests/conformance/expr.const/builtins-*.cpp` (11: constant
  evaluation, compile-fail and run), `tests/codegen/builtins_*.cpp` (5
  IR-diff programs), the `templates/builtins/has-builtin-*` probes, and
  three `bench/oven/builtins-*` entries.
- Gaps: `__builtin_FUNCTION` in a function with a deduced return type
  prints `auto` (clang prints the deduced type), and the
  `[T = int]` template-argument suffix clang appends inside templates
  is not produced. A `CXXDefaultInitExpr` does not yet take the
  location of the constructor that uses it (owner: WP-067b).

**Landed (WP-071): function templates.**
- `deduce_call` (`templ/deduce_call.rs`, §6.1) builds each candidate's
  specialization. Explicit arguments are checked and then substituted
  into P under an `ExplicitArgSubstitution` SFINAE frame. Too many of
  them is a `SubstitutionFailure` that carries clang's arity message.
  `deduce_explicit_only` is the same pipeline with no call: it is used
  for a function template-id that is not called ([over.over]/3,
  `Sema::resolve_single_template_id`). Mismatch notes print P as
  written, because `proto_param_types` returns the sugared prototype.
- `deduce_template_id` with `DERIVED_CLASS`: when no base matches, the
  result is the one for `A` itself (clang's
  `DeduceTemplateSpecArguments`). For example, "deduced conflicting
  types" survives a failed base search.
- Partial ordering (`templ/order.rs::more_specialized`, [temp.func.order])
  works as follows, without packs:
  - take the first `nargs` parameter types of each template;
  - strip references and top-level cv;
  - deduce the other template's parameters from them under SFINAE.

  The overload sets precompute the "more specialized" pairs
  (`order_template_candidates` into `OverloadCandidateSet::tmpl_order`)
  before `best_viable`. `better()` reads the pairs for /2.5.
- Template candidates come from named calls, member calls (`member.rs`,
  implicit and explicit object, `e.f<args>` via
  `Sema::set_member_template_args`), and non-reversed operator
  candidates, both non-member and member (`oper.rs`). A parsed `f<args>`
  is widened to the whole overload set (`widen_function_template_id`).
- Names: a specialization prints as `f<int>` in diagnostics, in Oven
  notes and in the "in call to" stack (`write_decl_name`,
  `kiln_oven::notes::write_name_for_diag`). Candidate notes say
  `candidate function [with T = int]`, except for a bad conversion on a
  non-dependent parameter (CWG1391), which says
  `candidate function template`, as clang does.
- Gaps, with owners:
  - packs: WP-072;
  - constraints and `requires`: WP-073;
  - CTAD: WP-071b;
  - reversed and rewritten operator template candidates ([over.match.oper]/3.4);
  - conversion function templates (`ics.rs`);
  - `&f` of a template against a target type, which needs deduction from
    a function type ([temp.deduct.funcaddr]);
  - explicit specialization and explicit instantiation of function
    templates; `TemplateParamObject`;
  - dependent explicit arguments on a member template-id of a
    non-dependent object.

  The [temp.deduct.call]/1 `initializer_list<P>` path deduces, but
  kiln cannot yet build an `std::initializer_list` object, so the test
  for it is an xfail.
- The `<type_traits>` probe went from 24 errors to 20. What remains:
  - 7 constrained partial specializations (WP-073);
  - 7 requires-expressions (WP-073);
  - 2 pack expansions (WP-072);
  - 4 unrelated to WP-071: `__int128_t` and its follow-ons.

### 11.3 Instantiation report and memory accounting

**Meter.** Each instantiation frame records, at push, `(Ast::bytes(),
node_count, Instant)`. At pop it computes the inclusive deltas. The
*self* deltas are the inclusive deltas minus the inclusive deltas of the
children, which accumulate in the parent frame.

**Aggregation** is per *template*: the primary template's `DeclId`, or
the member template's pattern.

```rust
pub struct InstRow { template: DeclId, kind: RowKind /*class,function,var,member,alias,concept*/,
                     count: u32, nodes: u64, bytes: u64, self_ns: u64, incl_ns: u64 }
```

**Always-on counters.** These are cheap:
- `count`, `nodes` and `bytes`;
- deductions (attempts, failures);
- satisfaction checks (cache hits);
- `templ.sfinae_discarded` bytes.

Timing is collected only with `-fkiln-instantiation-report`.

**MemReport rows:**
- `instantiation.retained` (ast.md §15);
- `templ.tables` (`TemplateTables` maps);
- `templ.caches` (alias, satisfaction, atom, subsumption, partial-order
  and pack-ref caches);
- `templ.satisfaction` (records);
- `templ.sfinae_discarded`.

**Text format** (`-fkiln-instantiation-report`, on stderr). Rows are
sorted by bytes, descending; the top 50 are shown, or all with `=all`.

```
===-- kiln template instantiation report: t.cpp --===
   count     nodes      bytes   self ms   incl ms  kind      template
    1025     21525     688800      3.10      9.80  static    R<N>::v   (t.cpp:1:49)
       …
  total: 1324 instantiations, 31232 nodes, 0.90 MiB retained, 12.4 ms
  deductions: 412 (37 failed)   satisfaction: 98 (61 cached)   sfinae discarded: 18.2 KiB
```

**JSON** (`=json`, one NDJSON object per row, then a totals row). The
keys use oven.md's report vocabulary:
`{"template":"R<N>::v","kind":"static","loc":"t.cpp:1:49","count":1025,"nodes":21525,"bytes":688800,"self_us":3100,"incl_us":9800}`.
Goldens mask `*_us`.

### 11.4 Bench gates (`bench/frontend`, spec/bench.md)

**Metrics added to each frontend case:**
- `inst.count` (exact; any change needs a baseline update in the PR);
- `inst.bytes` (±2%);
- `ast.bytes` (±2%);
- `sat.count`;
- wall time per bench.md.

**New cases:**
- `templates_deep.cpp` (depth 1000 static-member chain);
- `templates_wide.cpp` (10k small class specializations);
- `concepts_heavy.cpp` (constrained overload sets, deep subsumption);
- `fold_big.cpp` (a fold over 4096 elements);
- `libcxx_m3.cpp` (includes the 8 exit headers and instantiates their
  typical uses);
- the mirrorbind synthetic corpus, when WP-030 delivers it.

*Landed (WP-086).* `cargo xtask bench` passes kiln
`-fkiln-instantiation-report=json` for every entry of the `frontend`
group and records the totals row as `inst: {count, nodes, bytes}` in the
results and baselines (bench.md "Measurement", "Records"). `inst.count`
is judged exactly (`REGRESSED (inst)` on any increase, `improved` on a
decrease), `inst.bytes` against the bytes threshold (2%); a total that
differs between runs fails the entry. `--record` writes them into the
baselines and as the `kiln instantiations` column (`count / KiB`) of
`docs/status/perf.md`. Not yet: `ast.bytes` (the totals row has no
whole-TU AST size; adding one would churn every `inst-report:` golden on
any AST change) and `sat.count` (no satisfaction counter until WP-073);
`--clang-inst` below is not built.

**Laziness parity.** `cargo xtask bench --clang-inst` counts
`InstantiateClass` and `InstantiateFunction` events in clang 19's
`-ftime-trace -ftime-trace-granularity=0` output. The gate is
`kiln inst.count ≤ clang`.

## 12. Agent parallelization

```
WP-064 ─┐
WP-065 ─┴─ WP-069 parser ── WP-070 core ─┬─ WP-071 functions (+068) ─┬─ WP-072 packs
                                          │                           ├─ WP-073 concepts
                                          └─ WP-074 auto/builtins     └─ WP-075 generic lambdas
                                                         all ── WP-081 libc++ M3 gate
```

| WP | Owns (§11.1) | Consumes | Hands on | Stubs it must leave | Acceptance |
|---|---|---|---|---|---|
| 069 parser | parser `templ.rs` + hooks; `templ/actions.rs`; AST constructors | 064 `is_cxx_type_id`, 065 class parsing | patterns with dependent nodes; `classify_name` template variants; annotations | sema actions that build nodes but skip checks needing 070 (no instantiation, no arg conversion beyond kinds) | ≥40 `templates/parse/*.cpp` `ast` goldens of patterns vs clang; `<type_traits>` reaches 0 *parse* errors |
| 070 core | `templ/{mod,args,class,pending,stack,report}`, `subst/*`, `deduce`, `order` (core), `TemplateTables` | 069 | `Substituter`, `MultiLevelArgs`, `require_complete`, `get_*_spec`, the stack and SFINAE, the report, `ensure_body` | `deduce_call`, pack expansion, `check_satisfaction` and `TABLE` return the error "kiln: not yet implemented: <feature>" (`err_unimplemented_template_feature`), never a panic | ≥60 tests in `templates/class/`, `templates/alias/`, `templates/var/`, `templates/diag/` (stack, elision, depth); report goldens; `templates.cpp` bench baseline |
| 071 functions | `deduce_call`, `funcs`, `ctad` | 070, 068 (S3O candidate API) | `deduce_call`, `more_specialized`, `deduce_auto` hooks | CTAD beyond M3 scope gives a diagnostic | ≥60 tests in `templates/deduce/`, `templates/order/`, `templates/sfinae/`, `templates/ctad/`; all §10.4 notes golden-tested |
| 072 packs | `pack.rs`; the expansion sites in `subst/*` (sequenced after 071) | 071 | `expand`, folds, pack indexing, `__type_pack_element` core | — | ≥40 tests in `templates/packs/`, incl. `stress/fold-65536.cpp` |
| 073 concepts | `concepts/*` | 071 | `check_satisfaction`, `subsumes`, `requires` | — | ≥50 tests in `templates/concepts/`; all §8.7 note chains golden-tested |
| 074 auto/builtins | `builtins/*`, `auto.rs`; kiln_pp `__has_builtin` hookup | 070 (+S3C props for the class traits) | `TABLE`, the trait evaluators, `deduce_auto`, `decltype(auto)` | traits waiting on S3C props report unimplemented, and `__has_builtin` is false for them | one test per §9 row (positive and negative) plus a clang probe; `templates/auto/` ≥30 |

**Status after WP-070b.** WP-070 (a and b) has landed. The stubs it
leaves, with their owners, are listed under "Landed (WP-070b)" in §11.2:
- WP-071: `deduce_call` and function template `more_specialized`;
- WP-072: packs in deduction and in partial specializations;
- WP-073: constrained partial specializations.

Member explicit specialization needs a follow-up WP, because it
touches WP-067b's declarator paths.

**Rules.**
1. **Signatures are fixed.** WP-070 lands every file listed in §11.1 with
   the §11.2 signatures and stub bodies. Later WPs fill in bodies; a
   signature change needs a spec edit in the same PR.
2. **Shared edit sites.** The pack-expansion sites in `subst/*` are the
   only shared edits. WP-070 marks each one with a call to
   `self.expand_list(...)`, a stub that handles only non-pack lists.
   WP-072 implements it in `pack.rs`.
3. **Backlog sizing.** WP-070 has more than 5 deliverables (backlog
   lesson). The recommendation (Q8) is to split it:
   - **070a**: substitution core, the stack and SFINAE, alias templates,
     class instantiation without partials, the report;
   - **070b**: the deduction core, partial and explicit specializations,
     variable templates, static members and `ensure_body`.

## 13. Tests

### 13.1 Conformance matrix

| Section | Directory (`tests/templates/…`) | Min | Owner |
|---|---|---|---|
| [temp.pre], [temp.param] | `parse/`, `params/` | 25 | 069/070 |
| [temp.names], [temp.res] (typename, template, P0634, P0846) | `parse/`, `dependent/` | 30 | 069 |
| [temp.arg] (type, nontype incl. class-type and `auto`, template, P0522) | `args/` | 30 | 070/071 |
| [temp.type], [temp.over.link] | `equiv/` | 15 | 071 |
| [temp.class], [temp.mem], [temp.friend], [temp.spec.partial] | `class/` | 40 | 070 |
| [temp.alias], variable templates | `alias/`, `var/` | 20 | 070 |
| [temp.dep], [temp.dep.candidate] | `dependent/` | 25 | 070 |
| [temp.inst], [temp.explicit], [temp.expl.spec], [temp.point] | `inst/` | 30 | 070 |
| [temp.fct], [temp.deduct.*], [temp.func.order] | `deduce/`, `order/`, `sfinae/` | 60 | 071 |
| [temp.deduct.guide], [over.match.class.deduct] (M3 part) | `ctad/` | 10 | 071 |
| [temp.variadic], [expr.prim.fold], P2662 | `packs/` | 40 | 072 |
| [temp.constr], [temp.concept], [expr.prim.req] | `concepts/` | 50 | 073 |
| [dcl.type.auto.deduct], [dcl.type.decltype], [dcl.spec.auto] | `auto/` | 30 | 074 |
| [meta] builtin traits (§9) | `builtins/` | 1 per row | 074 |

**Standard examples.** The `[temp.*]` examples from the working draft are
extracted as tests. An example annotated `// error` becomes a
`compile-fail` test. Where clang 19 disagrees with the note, the test is
`xfail(clang)` and kiln follows clang.

### 13.2 Test kinds

- **`expect: ast`** with `ast-filter` on a specialization name. It
  compares `ClassTemplateSpecializationDecl` and specialization-function
  dumps, `SubstNonTypeTemplateParmExpr`, folds and concept-ids with
  `clang -Xclang -ast-dump`. `SubstTemplateTypeParm` sugar prints
  transparently (ast-dump.md §5).
- **`expect: compile-fail`** with `diag:` lines for the full note chains,
  covering every §10.1 frame, every §10.4 `Tdk` and every §8.7 note. It
  is run again with `-ftemplate-backtrace-limit=0/4` and with
  `-ftemplate-depth=16`.
- **The instantiation report** needs a new harness directive,
  `inst-report: <golden.json>`. It compares the JSON with `*_us` masked.
  WP-070 extends harness.md.
- **SFINAE/hard-error matrix.** Every case in [temp.deduct.general]/8,
  plus the pairs of class-instantiation errors inside deduction
  (expected: a hard error with the full stack).
- **Stress.**
  - `template-depth-1024.cpp`: within the stack budget.
  - `fold-65536.cpp`.
  - `wide-10k.cpp`.
  - `subsume-blowup.cpp`, which exercises D5.
  - `alias-chain-1000.cpp`.
- **Differential.** `cargo xtask bench --clang-inst` (§11.4).

### 13.3 libc++ M3 exit set and feature survey

The survey preprocessed the 8 exit headers with clang 19 (`-std=c++26
-stdlib=libc++ -E -P`), which gives 11,358 lines (`clang -fsyntax-only`
takes 0.12 s). Line counts:

| Feature | Lines | Feature | Lines |
|---|---|---|---|
| `template <` heads | 1612 | partial specializations (class-head template-id) | ~227 |
| explicit specializations `template <>` | 100 | alias declarations `using X =` | 340 |
| variable templates `inline constexpr …` | 142 | `static constexpr` members | 269 |
| `concept` | 138 | `requires` (clauses + expressions) | 208 (95 `requires(`) |
| `enable_if` | 119 | `typename` | 257 |
| `decltype(` | 162 | `noexcept(` | 116 |
| `::template` / `.template` / `->template` | 27 | template template params | 14 |
| `if constexpr` | 55 | `...` (packs, folds, varargs) | 329 |
| fold expressions | 17 | `sizeof...` | 10 |
| `decltype(auto)` | 7 | deduction guides | 11 |
| `explicit(bool)` | 9 | `operator<=>` | 12 |
| `friend` | 57 | `consteval` | 1 |
| `__make_integer_seq` / `__type_pack_element` | 2 / 2 | pack indexing, `template<auto>` | 0 / 0 |

The headers use 143 distinct builtins for which `__has_builtin` is true
(§9). Of these:
- 50 are type traits (`__is_*`, `__has_*`, `__array_extent`);
- 15 are type transforms;
- 2 are builtin templates;
- 76 are `__builtin_*` functions.

Each WP's acceptance includes the drop in errors on
`kiln -fsyntax-only` over the 8 headers. The status page is
`docs/status/libcxx-m3.md`, maintained by WP-081. The baseline today is
490 errors on `<type_traits>`, 436 of them template-related
(docs/status/libcxx-syntax.md).

## 14. Open questions

Each question has a recommendation (R).

1. **Q1: Concepts milestone.** ROADMAP.md puts concepts in M4, but the M3
   exit headers need them (138 `concept`, 208 `requires`) and BACKLOG
   puts WP-073 in M3. **R:** concepts are M3; edit ROADMAP M4.
2. **Q2: Pack indexing (P2662).** ROADMAP M4 lists it. **R:** M3 (§7.4),
   because it is cheap, clang 19 is an oracle, and it shares code with
   `__type_pack_element`.
3. **Q3: Non-dependent subtree sharing (§3.3, D4).** It needs a new
   `ExprDeps::LOCAL` bit, an ast.md §5 change. **R:** implement it in
   WP-070 behind `-fno-share-nondependent`, measure it with the report,
   and keep it if it saves ≥15% of `inst.bytes` on `libcxx_m3.cpp`.
   **WP-070a: deferred to WP-071.** No code, flag or `LOCAL` bit landed,
   and ast.md §5 is unchanged. The measurement cannot be made yet:
   `libcxx_m3.cpp` does not exist and kiln cannot compile the M3
   headers, and WP-070a instantiates no function-template bodies, where
   the shared subtrees are. It instantiates only member functions,
   default member initializers, static-member initializers, array
   bounds and `static_assert`s. WP-071 measures it once function bodies
   and `libcxx_m3.cpp` exist, with the same ≥15% rule.
4. **Q4: Stack strategy (D1).** **R:** a dedicated sema thread with a
   256 MiB reserved stack, plus `StackGuard` with a 1 MiB red zone. The
   alternative is a `stacker`-style segmented stack, which needs `unsafe`
   in a dependency and conflicts with the unsafe policy.
   **Decided (WP-086):** the dedicated thread and `StackGuard` as
   recommended, with a **1 GiB** default: a debug build needs more than
   256 MiB for depth 1024, and the reservation costs only address space.
5. **Q5: Oven reentrancy (R-OVEN-1) and `EvalKind::Constraint`.** **R:**
   the Oven owner confirms that nested evaluation from bridge callbacks
   works (`ensure_body(…, &mut Oven)`). Otherwise fall back to "no
   instantiation from `ensure_body`" (§4.11) and accept that calls
   through computed pointers to never-referenced specializations fail.
6. **Q6: Hash-consed frozen values (R-OVEN-2)** for class-type NTTP
   identity. **R:** required in WP-061b's heap design.
7. **Q7: When constexpr function templates are instantiated.** **R:**
   immediately at odr-use, as clang (§6.5). Deferring them to
   `ensure_body` would change the order of diagnostics against the
   goldens.
8. **Q8: Split WP-070** into 070a and 070b (§12). **R:** yes. Also move
   CTAD out of 071 into a small WP-071b if 071 has more than 5
   deliverables.
9. **Q9: WP-069 dependencies** must add WP-064, because template argument
   disambiguation needs the tentative type-id scan. **R:** edit BACKLOG.
10. **Q10: `__has_builtin` staging.** **R:** answer from `TABLE`'s
    implemented column, so libc++ uses its library fallbacks until a
    builtin lands. WP-081 requires every §9 row to be implemented.
11. **Q11: Report flag name and counters.** **R:**
    `-fkiln-instantiation-report[=json|all]`. Counts and bytes are always
    on; timing only with the flag.
12. **Q12: Order of the constraint check versus the function-type
    substitution (CWG2369).** **R:** follow clang 19 (after
    substitution), and revisit when the clang oracle moves.
13. **Q13: A deduction memo cache** keyed by `(ft, argument
    type/category signature)`. **R:** defer to M7 and decide from report
    data.
14. **Q14: Subsumption clause cap (D5).** **R:** 65,536, with an error
    rather than silent "not subsumed".
15. **Q15: Reserved depths** (`AUTO_DEPTH = 0x7FFD`,
    `SYNTH_DEPTH = 0x7FFE`). **R:** record them in types.md §5.3
    (`CanonTemplateTypeParm` has 15 depth bits); user templates are
    limited to depth < 0x7FF0.
16. **Q16: Deep folds (R-WALK).** **R:** make the dumper, the Oven
    lowering and codegen iterative along binary spines in M3, and test
    with `fold-65536.cpp`. Otherwise kiln depends on the stack guard and
    fails where clang succeeds.
17. **Q17: Deferred C++26 features**: structured-binding packs (P1061),
    fold-expanded constraints (P2963), variadic friends (P2893) and
    concept template parameters (P2841). **R:** M4/M5, when the clang
    oracle supports them.

### Supervisor decisions

| Q | Decision (2026-10-28, binding) |
|---|---|
| Q1 | Concepts are M3. ROADMAP M3/M4 edited. |
| Q2 | Pack indexing (P2662) is M3, in WP-072. ROADMAP edited. |
| Q3 | Implement non-dependent subtree sharing in WP-070a behind `-fno-share-nondependent`; `ExprDeps::LOCAL` is added to ast.md §5 by that WP; keep it only if the report shows ≥15% `inst.bytes` saved on `libcxx_m3.cpp`, else remove the code (no dead flags). WP-070a could not measure it (§14 Q3), so it moves to WP-071 with the same rule; nothing landed. |
| Q4 | Dedicated sema thread (1 GiB reserved stack by default — 256 MiB until WP-086 measured debug builds — `-fkiln-stack-size`, `std::thread::Builder::stack_size`) plus `StackGuard` with a 1 MiB red zone. The whole front end (parser + sema + Oven) runs on that thread, since the parser calls sema. No segmented stacks, no `unsafe`. |
| Q5 | R-OVEN-1 accepted: the Oven supports nested `evaluate` from bridge callbacks (a stack of `EvalContext`s; the frame space uses marks per context and is reset only when the outermost context ends; budget shared, `depth` counts nested contexts). It lands in **WP-061b** (which already reworks `mem.rs` reset) with `EvalKind::Constraint`; `ensure_body(&mut self, f, oven: &mut Oven)` is the contract. No fallback. |
| Q6 | R-OVEN-2 accepted: frozen values are hash-consed by content in `ConstStore` (WP-061b, `frozen_bytes` counts only new values). |
| Q7 | Constexpr function templates are instantiated immediately at odr-use, as clang. |
| Q8 | WP-070 splits into 070a/070b as §12; CTAD moves to WP-071b. BACKLOG edited. |
| Q9 | WP-069 needs WP-064. BACKLOG edited. |
| Q10 | `__has_builtin` answers from `TABLE`'s implemented column; WP-081 requires every §9 row implemented. |
| Q11 | `-fkiln-instantiation-report[=json\|all]`; counts and bytes always on, timing only with the flag. The JSON schema is a report golden under `tests/conformance/templates/report/`. |
| Q12 | Follow clang 19 (constraint check after substitution of the function type). |
| Q13 | Deduction memo cache deferred to M7, decided from report data. |
| Q14 | Subsumption cap 65,536 clauses, diagnosed as an error. |
| Q15 | Reserved depths recorded in types.md §5.3; user templates limited to depth < 0x7FF0. |
| Q16 | Dumper, Oven lowering and codegen are iterative along binary spines in M3, tested by `fold-65536.cpp` (owner: WP-072 adds the test; each walker's owner crate fixes its walker; the Oven's `compile.rs` is WP-076's). |
| Q17 | P1061, P2963, P2893, P2841 deferred to M4/M5. |
