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
| CTAD | implicit guides from constructors and constructor templates, the copy guide, user-declared guides, aggregate CTAD (P1816), alias CTAD (P1814), inherited-constructor guides (P2582, §6.6.1), guides of member templates of instantiated class templates, [over.match.list] phase 1 (WP-071b, WP-134) | — |
| Packs | type, value and template packs; expansion in every context in §2.7; folds; `sizeof...`; pack indexing P2662 | expansion statements P1306 (M6, with reflection) |
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
- End-of-TU instantiation is a worklist (`pending`), not recursion: the
  drain keeps an explicit stack of saved queues to visit instantiations
  depth-first, in clang's `GlobalEagerInstantiationScope` order
  (WP-172, spec/codegen.md §2a), each scope under its requesting
  definition's frame (§10.2, WP-238).

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
| D7 | rejects an out-of-line member of a member partial specialization of an implicit specialization (`template<> template<class T, char c> void O<int,char>::A<T*,c>::f() {}`) with "nested name specifier 'O<int, char>::A<T *, c>' for declaration does not refer into a class, class template or class template partial specialization" (clang 19 and 22) | accepts it as the member of that partial specialization (§4.5) | [temp.spec.partial.member]/1, [temp.mem]/1; clang fails to match the qualifier against the partial specialization's own head (WP-401 decision) |

Apart from D7, none of these changes the accept/reject result of a program
that clang 19 compiles within its own limits.

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
template-template-param := template-head ('class'|'typename'|'auto') '...'? identifier? ('=' id-expression)?
                        | 'template' '<' template-parameter-list '>' 'concept' '...'? identifier? ('=' id-expression)?
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
    'X' should be empty ('template<>')"
  The lists are matched against every enclosing scope slot, explicit
  specializations included (WP-161): a member partial specialization of
  `Outer<int>` written with `template<class T> template<class U>` gets
  the second error, and its declaration is then dropped
  (`take_template_head` reports `head_mismatch`).
- **Headers for an explicitly specialized class** ([temp.expl.spec]/17,
  WP-401). A member of an explicit specialization `O<int, char>` is
  defined with only its own head: `template<char c> int O<int,
  char>::f()`. The parser hands sema the locations of the leading
  `template<>` headers (`act_explicit_spec_headers`, namespace scope
  only), and `check_extra_spec_headers` (`templ/mod.rs`, clang
  `MatchTemplateParametersToScopeSpecifier`) consumes one header per
  enclosing implicit-specialization or class-template slot of the
  qualifier and stops at the first explicit specialization. If more than
  one header is left (the last belongs to the declaration itself):
  - all left-over headers empty: "extraneous template parameter list in
    template specialization" at the first extraneous `template`. At
    compat 22 it is a default-error extension in group
    `extraneous-template-head` (`sema_template_spec_extra_headers`); at
    compat 19 a plain warning with no group
    (`sema_template_spec_extra_headers_clang19`, chosen by sema: kind and
    group differ, so it is not a `_19` mirror). The declaration is kept.
  - some left-over header non-empty: the error "extraneous template
    parameter list in template specialization or out-of-line template
    definition" and the declaration is dropped (`head_mismatch`).
  Either adds the note "'template<>' header not required for
  explicitly-specialized class 'O<int, char>' declared here" at the
  innermost explicit specialization enclosing the qualifier (only when one
  exists; a free function gets no note). Tests:
  `temp.expl.spec/extraneous-template-head{,-clang19,-warning,-silenced}.cpp`.
- **Empty `template<>` before a non-template** (clang
  `err_template_tag_noparams` / `err_template_variable_noparams`,
  WP-401). `template<> struct D {};` (with `D` not a template) and
  `template<> int v = 0;` (with `v` not a variable template) error
  "extraneous 'template<>' in declaration of struct 'D'" /
  "... of variable 'v'" (`sema_template_tag_noparams`,
  `sema_template_variable_noparams`), and the entity is declared as a
  plain non-template so later uses resolve. The parser detects the tag
  shape (`at_plain_tag_decl` in `kiln_parse/src/templ.rs`); the variable
  shape is caught in `act_declarator` when an explicit-spec head meets a
  non-template, non-function name. Test:
  `temp.expl.spec/extraneous-empty-template-head.cpp`.

**Defaults** are kept unparsed only where [temp.param]/15 requires
delayed meaning; in practice they are always parsed immediately.
- A default may name earlier parameters. A parameter's locus is after
  its complete template-parameter ([basic.scope.pdecl]/9): its own
  default does not see it, so `template <class T = T>` names an outer
  `T` (libc++ `template <class _Tp = _Tp>` in a member template of a
  class template with a parameter `_Tp`). The parser declares the name
  in the template-parameter scope after the default
  (`Sema::act_on_template_param_end`, which also reports the
  [temp.local]/6 shadowing error; WP-295,
  `tests/conformance/basic.scope.pdecl/template-param-own-default.cpp`).
- Redefining a default is an error: "template parameter redefines default
  argument".
- A non-type parameter's decl-specifiers are checked as clang's
  `CheckValidDeclSpecifiers` does (`act_on_param_declarator`,
  `DeclSite::TemplateParam`; [temp.param]/2): a storage class,
  `thread_local`, `inline`, `constexpr`/`consteval`/`constinit`,
  `virtual` or `explicit` is "invalid declaration specifier in template
  non-type parameter" once per specifier, at it, instead of the
  function-parameter diagnostics (`template<static int a...>` reports the
  misplaced `...` first). C11 `_Noreturn` is one more such specifier,
  last in that order. `friend` is not: outside a class it is "'friend'
  used outside of class", reported by the parser while reading the
  decl-specifiers and dropped (any parameter, namespace or block scope
  declaration), so the parameter stays usable. Tests:
  `temp.param/nttp-invalid-decl-specifier.cpp`,
  `temp.param/nttp-friend-noreturn-specifiers.cpp`,
  `class.friend/friend-outside-class.cpp`,
  `dcl.attr.noreturn/c11-noreturn-keyword.cpp` (`_Noreturn` elsewhere: the
  function's `[[noreturn]]`, "'_Noreturn' can only appear on functions"
  on anything else, "'_Noreturn' is a C11 extension" under `-pedantic`).
- A non-type parameter of class type needs a complete type ([temp.param]/6,
  clang's `RequireStructuralType`): "non-type template parameter has
  incomplete type 'S'" with the forward-declaration note, also when the
  type comes from a prior argument (`template<class T, T t>` with
  `T = S`, under the "while substituting prior template arguments" note).
  `template<static class T>` declares such a parameter of the incomplete
  class `T`. Test: `temp.param/nttp-incomplete-class-type.cpp`.
- A template template parameter's template-head is followed by `class` or
  `typename` ([temp.param]/1). Without one, clang 19 always reports
  "template template parameter requires 'class' or 'typename' after the
  parameter list" ("requires 'class'" before C++17); clang 22 only before a
  name, `,`, `>`, `>>` or `...`, or at `struct` (consumed). Then a token that
  cannot be the parameter's name (`static`, `int`, `union`) is "expected
  identifier" and the parameter is dropped; the list goes on after a `,`
  (`ParseTemplateParameterList`). A list left empty by dropped parameters
  makes a following plain class declaration a non-template with
  "extraneous 'template<>' in declaration of struct 'D'". Where a
  parameter does recover without its key, clang 19 (and kiln at compat
  19) makes it a type template template parameter. At compat 22 it names
  no template (`act_on_keyless_template_template_param`, clang 22's
  recovery, WP-409): every argument for it is "template argument does not
  refer to a class or alias template, or template template parameter"
  with "template argument refers to a class template 'A', here", and its
  uses are not template names (`T<int>` is "no template named 'T'", `T x`
  "unknown type name 'T'"). Tests:
  `temp.param/template-template-param-missing-key{,-clang19,-cxx14}.cpp`,
  `temp.param/template-template-param-keyless-recovery{,-clang19}.cpp`.
- A template template parameter's default argument names a class
  template, an alias template or a template template parameter as a whole
  template-argument ([temp.param]/10, clang's
  `ParseTemplateTemplateArgument`). Anything else (a type parameter, a
  class, a template-id `K<int>`, a qualified non-template) is "default
  template argument for a template template parameter must be a class
  template" at the token after what was parsed (at a template-id's start),
  and the parameter is skipped to its `,`/`>`. At compat 22 a variable
  template or concept is a template name of the wrong kind (P2841):
  "template argument does not refer to a class or alias template, or
  template template parameter" with its "refers to" note
  (`act_on_wrong_kind_ttp_default`, WP-422); at 19 it is the "must be a
  class template" error after the name. Tests:
  `temp.param/ttp-default-not-class-template{,-clang19}.cpp`.
- A template template parameter whose template-head has no parameters
  (`template<> class U`) is "template template parameter must have its
  own template parameters" at its name (at the next token when unnamed);
  the parameter is kept. Test: `temp.param/template-template-param-no-params.cpp`.
- Defaults are stored in the cold "template default args" map
  (ast.md §8.1).

**Non-type parameters that are pack expansions** ([temp.param]/17,
[temp.variadic]/5, WP-385). In a member template of `O<Ts...>`,
`template<Ts... a>` declares one non-type parameter whose type is the
expansion `Ts...`. Substituting the enclosing arguments
(`TemplState::expanded_packs`, `member.rs` `nttp_param_ty`) makes it one
parameter per element of `Ts`: `O<int, char>::A` takes `<int, char>`,
`O<>::A` takes none. Argument checking (`args.rs`
`check_expanded_pack_arg`, `nttp_elem`) converts each argument to its
element type, and wrong arity is clang's "too few/too many template
arguments". Deducing a member function template whose explicit
arguments leave such an expanded pack incomplete is
`Tdk::IncompletePack`, clang's note "deduced too few arguments for
expanded pack 'a'; no argument for 2nd expanded parameter in deduced
argument pack <1>". Partial specializations over it work
(`template<Ts... a> struct Is<A<a...>>`). Such a parameter is not itself
a parameter pack ([temp.param]/14), so "template parameter pack must be
the last template parameter" does not apply to it: `template<Ts... a,
int n>` is valid. Arity is checked even when the argument list is
dependent (`check_expanded_pack_arity`, as clang's
`CheckTemplateArgumentList`): each expanded parameter takes at most one
argument per element, a pack expansion argument counting as one, and
"too few" is excused once a pack expansion was taken; so `O<>` with
`Is<A<a...>>`, or `A<1, u...>` for a one-element expanded pack, is "too
many template arguments for class template 'A'" in the instantiation.
The template is named unqualified there (`diag_template_arity` uses
the declaration's own name, as clang prints a `NamedDecl`), not
`O<int, char>::A`. Tests: `temp.variadic/p5-member-nttp-pack-expansion.cpp`,
`temp.variadic/p5-member-nttp-pack-arity.cpp`,
`temp.variadic/p5-member-nttp-pack-dependent-arity.cpp`,
`temp.variadic/p5-member-nttp-pack-alias-arity.cpp`.

**Local classes** ([temp.mem]/2, WP-385). A local class, or a class
nested in one, has no member templates. A member function, class,
variable or alias template there is "templates cannot be declared
inside of a local class" (`sema_template_decl_in_local`) at the
`template` keyword (`check_template_in_local_class`). An abbreviated
function template (`void f(auto)`, [dcl.fct]/22) has no `template`
keyword: it is diagnosed where its declaration starts, a leading
attribute included (clang 22; clang 19 gives no location). A friend
function template is "templates can only be declared in namespace or
class scope". The member function template stays declared but invalid
(its body is still checked), so a call finds no candidate: "no matching
member function for call to 'f'" without notes. A generic lambda in a
member function is not a member template and is accepted. Tests:
`temp.mem/p2-local-class-member-template.cpp`,
`temp.mem/p2-local-class-abbreviated-template.cpp`.

#### 2.1.1 Concept and variable template template parameters (P2841R7)

**Kinds** ([temp.param]/2-3). A template template parameter is a *type*,
*variable* or *concept* template template parameter, recorded as
`TemplateParmInfo::ttp_kind` (`TtpKind::{Type, Variable, Concept}`,
`Sema::ttp_kind`; `kiln_sema/src/templ/ttpkind.rs`). The parser takes
`auto` (variable-tt-parameter) or `concept` (concept-tt-parameter) where it
takes the `class`/`typename` key, only at `-fclang-compat=22` (clang 19
reports the missing key, unchanged); before C++26 each is the error
"concept template parameter is a C++2c extension" /
"variable template template parameter is a C++2c extension"
(`parse_ext_concept_template_param`,
`parse_ext_variable_template_template_param`, clang 22's
`ext_cxx26_*`, error by default) and the parameter is still declared.
Packs, unnamed parameters and defaults (`= Int`, `= sz`) parse as for a
type template template parameter. [temp.param]/5: a concept template
parameter shall not have associated constraints; a requires-clause in its
`template<...>` or a type-constraint on one of its parameters is
"concept template parameter cannot have associated constraints"
(`sema_concept_template_param_constraints`, at the constraint). clang 22
accepts both silently: kiln follows the wording here. A variable template
template parameter's template-head may carry a requires-clause.

**Concept heads** ([temp.param]/14). A concept's template-head is checked
like a primary class template's for packs: a template parameter pack
before the last parameter is "template parameter pack must be the last
template parameter" (`check_template_param_list` with
`DeclKind::Concept`), so `template <class T, template <class> concept...
Cs> concept all_of` is fine and `template <template <class> concept...
Cs, class T>` is not. Like clang 22, the rule that a default argument be
followed only by defaults is not applied to a concept's head (clang
checks it only for the "ordered" kinds). Test
`temp.param/p14-concept-pack-last.cpp` (WP-771).

**Names** ([temp.names], [temp.param]/8). `classify_name` reports a concept
template parameter as `NameClass::Concept` and a variable template
template parameter as `NameClass::VarTemplate`, so `C<T>` parses as a
concept-id, `C auto` and `template <C T>` as type-constraints, and `V<T>`
as a variable template-id. In a template-argument list, a concept name or
variable template name for a parameter of that kind is a template
argument (`TemplateName`); for a type template template parameter it stays
the clang 22 error of §2.2.

**Uses before substitution.** `C<args>` for a concept template parameter
`C` is a `ConceptSpecializationExpr` whose decl is the parameter;
`V<args>` for a variable template template parameter is an
`UnresolvedLookupExpr` whose single decl is the parameter with the
arguments in `expr_targs` (`ttp_template_id_expr`). Both are
value-dependent (and `C<args>` is `bool`), and both carry the parameter
pack for [temp.variadic] (`expr_packs` visits the parameter and the
arguments, so `Cs<T>...` and `(Cs<T> && ...)` expand). A type-constraint
`C T` / `C auto` names the parameter as its concept.

**Substitution** (`Sema::ttp_template_id`). Substituting the parameter
yields the argument template (`Substituter::template_name`), and the use
is rebuilt as that template's template-id with the substituted arguments:
a checked concept-id ([temp.constr.concept]/2, satisfaction memoised on
the substituted concept and arguments, §8.3) or a variable template
specialization (`DeclRefExpr` to the instantiated variable), which the
Oven reads like any other ([expr.const]). A pack of concept or variable
templates substitutes element-wise through the expansion.

**Arguments** ([temp.arg.template]/1, /3). A template-argument for a
type template template parameter names a class or alias template, for a
concept template parameter a concept, for a variable template template
parameter a variable template (or, each time, a template template
parameter of the same kind; `template_arg_kind`). Otherwise clang 22's
"template argument does not refer to a {class or alias template|variable
template|concept}, or template template parameter"
(`sema_template_arg_not_valid_template`, at the argument), with the note
"template argument refers to a {..} '{name}', here" at the template. A
default argument of the wrong kind is rejected the same way and dropped.
Matching of an argument of the right kind is P0522's "at least as
specialized" check (§11.2, `templ/ttarg22.rs`) unchanged: the
parameter's template-head is formed as arguments to the argument template, so a defaulted or pack
parameter of the argument absorbs them and an argument that cannot accept
them is "too few/many template arguments for concept 'Same'" with the
"different template parameters" notes, as in clang 22. The kinds of the
parameters inside nested template template parameters are compared too
([temp.arg.template]/3 with [temp.param]/1, WP-771; `ttp22_arg_fits`):
for `template <template <template <class> concept> class> class P`, an
argument whose corresponding parameter is `template <class> class` or
`template <class> auto` is the same "does not refer to a ..." error at the
argument's parameter, with clang 22's notes. Test
`temp.arg.template/p1-nested-ttp-kind-mismatch.cpp`.

**Deduction** ([temp.deduct.type]/8-9). A `TemplateName` argument in P
that names a concept or variable template template parameter deduces the
argument's template like a type template template parameter (packs
included): `template <template <class> auto V> int pick(Q<V>)`, called
with `Q<sz>{}`, deduces `V = sz`.

**Equivalence** ([temp.over.link]/6.1): §6.8.

**Unexpanded packs** ([temp.variadic]/12). A requires-clause
(`act_on_requires_clause`, from `parse_requires_clause`) and a concept's
constraint-expression (`act_concept`) are checked like a full-expression:
`requires C<Ts>` or `requires Cs<T>` with `Ts`/`Cs` unexpanded is
"expression contains unexpanded parameter pack 'Ts'" at the expression's
start, and the clause becomes a `RecoveryExpr` (never satisfied). Tests
`temp.variadic/p12-unexpanded-requires-clause.cpp`,
`temp.variadic/p12-unexpanded-requires-clause-concept-pack.cpp`
(WP-771).

In a lambda's declarator ([temp.variadic]/5, WP-776; clang's
`DiagnoseUnexpandedParameterPacks` with the lambda's `LocalPacks`):
- the packs the lambda declares (its template parameter packs and
  function parameter packs) named unexpanded in its template-head's or
  trailing requires-clause are "requires clause contains unexpanded
  parameter pack 'Us'" at the clause's begin, and in a template
  parameter's default argument "default argument contains ...";
- an enclosing template's pack named there (`[]<class U>() requires
  C<U, Ts> {}...`) makes the lambda-expression contain it, so a pack
  expansion around the lambda expands it, and without one it is
  diagnosed at the enclosing full-expression or initializer. The
  lambda's constraints, checked when its call operator is used, see the
  expansion's element of each such pack (`lambda_constraint_levels`).
Tests `temp.variadic/p5-lambda-requires-clause-outer-pack.cpp`,
`p5-lambda-requires-clause-outer-pack-unsatisfied.cpp`,
`p5-lambda-template-head-requires-outer-pack.cpp` (`requires: clang<22`:
clang 22 mis-evaluates the template-head form), and
`p12-unexpanded-lambda-declarator.cpp`.

**Feature macro.** `__cpp_template_parameters` is 202502L at `-std=c++26`
and `-fclang-compat=22` (`kiln_pp` `kiln_features`, WP-771); at
`-fclang-compat=19` the forms are rejected and the macro is undefined.
clang 22 implements the paper without defining it. Tests
`cpp.predefined/template-parameters-macro*.cpp`.

Tests: `temp.param/concept-variable-ttp.cpp`,
`temp.param/concept-variable-ttp-cxx23.cpp`,
`temp.param/concept-variable-ttp-defaults-errors.cpp`,
`temp.param/concept-ttp-requires-clause.cpp`,
`temp.arg.template/ttp-kind-mismatch.cpp`,
`temp.arg.template/concept-variable-ttp-p0522.cpp`,
`temp.deduct.type/concept-variable-ttp-deduction-run.cpp`,
`temp.param/ir-concept-variable-ttp-mangling.cpp` (spec/abi.md).

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

**Before C++20: a non-template followed by template arguments**
(clang's `checkPotentialAngleBracket` /
`diagnoseExprIntendedAsTemplateName`, WP-385). Before C++20 an
unqualified name whose lookup finds functions is not assumed to be a
template; one whose lookup finds nothing still is, in every mode
(clang's `TNK_Undeclared_template`). When lookup finds exactly one
declaration that is not a template (a variable, enumerator, parameter,
non-template function, or member), and `<` is followed by `>` or by an
unambiguous type-id and then a closing `>` before `;`
(`Parser::potential_angle_close`), the list is skipped and the error is
"'f' does not name a template but is followed by template arguments"
(`sema_non_template_in_template_id`) at the name, with the note
"non-template declaration found by name lookup" at the declaration
(`Sema::potential_template_name`,
`Sema::diagnose_expr_intended_as_template_name`). The expression becomes
an error, and a following `(args)` is still parsed. The expression is
also invalid as clang's `ExprError` (`Parser::expr_invalid`): the rest of
a `= initializer` is skipped to the next declarator's `,` or the `;`, and
an expression statement to its `;`, with no "expected ';'" for a
following `{}` (`int q = p<long>(3) + v<S>{};`). In C++20 the same
spelling on an overload set of functions is the P0846 reading above.
Before C++20 a call `f<args>(…)` whose `f` ordinary lookup did not find
(only argument-dependent lookup found its templates) is clang's
extension warning "use of function template name with no prior
declaration in function call with explicit template arguments is a C++20
extension" (`sema_ext_adl_only_template_id`, `-Wc++20-extensions`) at the
name, after the call's own diagnostics, also when overload resolution
fails (clang's `ActOnCallExpr` check of the callee). A dependent call
warns at the definition and again per instantiation (`per_instantiation`,
spec/diag.md §7.4); a non-dependent one in a template once. A call where
ADL finds no template is an undeclared identifier with no warning.
From C++20 the same points get the off-by-default compatibility warning
"... is incompatible with C++ standards before C++20"
(`sema_cxx17_compat_adl_only_template_id`, group `pre-c++20-compat`, also
enabled by `pre-c++20-compat-pedantic`; clang 19 says "no prior function
template declaration", its `_19` row; WP-401).
Tests: `temp.names/adl-only-template-id-cxx17.cpp`,
`temp.arg.explicit/pre-cxx20-compat-adl-only-template-id{,-clang19}.cpp`,
`temp.arg.explicit/adl-only-template-id-cxx20-silent.cpp`,
`temp.names/non-template-followed-by-args-recovery.cpp`,
`temp.names/non-template-followed-by-args-cxx17.cpp`,
`temp.names/non-template-followed-by-args.cpp`.

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

A trailing `...` wraps the argument in a pack expansion. A
TemplateExpansion `TT...` for a template template parameter (a partial
specialization's `E<B, Bs...>`) is checked by its pattern and kept as
the expansion. Test:
`temp.variadic/template-template-pack-expansion-partial-spec-run.cpp`.

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

A qualifier naming a class that is declared in a dependent context but is
not the current instantiation where it is used ([temp.dep.type]/1) makes
the name dependent too: a local class `L` of a function template named
after its definition (`L::U x;` at block scope), or a member class `N` of
a class template named in a block of a member function outside `N`
(clang's `computeDeclContext` gives no context for it). In a context
where `typename` is required (block scope, a condition, a for-init, a
function parameter outside the implied contexts above), a type found
through such a qualifier is "missing 'typename' prior to dependent type
name 'L::U'" at the qualifier (`Sema::is_unentered_dependent_class`,
checked by `kiln_parse` `check_missing_typename_in_class` from the
qualified type arm of `parse_type_name_spec`). Kiln still finds the type
in `L` and recovers as if `typename` had been written. Inside `L`'s own
definition (its members, its declarators' scope) `L` is the current
instantiation and no `typename` is needed. Clang parses `L::U *p;` and
`sizeof(L::U)` at block scope as expressions; kiln parses them as types
(a gap). Test: `tests/conformance/temp.res.general/missing-typename-unentered-class.cpp`.

A parameter-declaration of a function declared at namespace or block
scope with an unqualified declarator-id (`template <class T> void
f(X<T>::m x);`) is such a context (WP-287): the parser sets
`param_typename` while parsing that parameter's
decl-specifier-seq (`parse_param_clause`, `DeclSite::Plain`), and the
error is at the first token of the name. A namespace-scope deduction
guide's first parameter is implicit, as in clang, whose
constructor-declarator lookahead annotates that parameter only; its other
parameters need `typename` (WP-291). After the error the declaration is
recovered as if `typename` were written, but it is invalid in clang:
`Declarator::missing_typename` puts the function in
`Sema::missing_typename_fns`, and a qualified (out-of-line) redeclaration
skips it, so `template <class T> void N::f(X<T>::m) {}` is "out-of-line
definition of 'f' does not match any declaration in namespace 'N'"
whatever its parameter says. Calls and unqualified redeclarations still
find it. Whether `g(X<T>::m)` is a parameter at all is the tentative
parse's decision (spec/parse.md §2.1): `void g(X<T>::m);` is a variable
(`tests/conformance/temp.res.general/missing-typename-parameter.cpp`,
`missing-typename-invalid-decl.cpp`,
`tests/conformance/dcl.ambig.res/dependent-qualified-param-or-init.cpp`).

**Deduction guides** ([temp.deduct.guide]/3, WP-291): every template
parameter of a deduction-guide template must be deducible from the guide's
parameter types, be a pack, or have a default argument. `act_on_deduction_guide`
deduces each parameter type against itself (`check_guide_deducible`); a
parameter left null appears only in non-deduced contexts
([temp.deduct.type]/5), and clang's "deduction guide template contains a
template parameter that cannot be deduced" is reported at the template name
with a "non-deducible template parameter" note per parameter. The guide
becomes invalid (`tests/conformance/temp.deduct.guide/non-deducible-parameter.cpp`).

**`template`** is needed after `::`, `.` or `->` when the qualifier or
object is dependent and not the current instantiation, as in
`x.template f<int>()` and `T::template X<U>`. Without it, `<` is
less-than. When the name is followed by `<`, clang's diagnostic
`err_missing_dependent_template_keyword` ("use 'template' keyword to
treat 'f' as a dependent template name") is emitted and the parser
recovers as if `template` had been written. The diagnostic and the
recovery apply only when nothing but a list fits: the `<` is followed
by `>` or begins with an unambiguous type-id (clang's
`isTemplateArgumentList` returning True). In a nested-name-specifier,
`if_c<rank<T>::value < rank<U>::value, U, T>::type` compares the two
values (`parse_nested_name_upto`; WP-804,
`tests/conformance/temp.names/dependent-member-less-than-before-scope.cpp`).

At instantiation, `T::template f<A>` names a template when the lookup of
`f` in the substituted `T` finds any template: an overload set that also
has non-template functions resolves the template-id over the whole set
(`member_template_named`; WP-792,
`tests/conformance/temp.names/template-kw-mixed-overload-set.cpp`). Only
a set without a template gets "'f' following the 'template' keyword
does not refer to a template".

### 2.4 Dependent nodes built at parse time

All of these are built by sema actions, which compute dependence as in
§3.2.

| Syntax (in a dependent context) | Node |
|---|---|
| `T::x`, `X<T>::x` or `X<T>::operator@` (not the current instantiation) in an expression; also `D::x` / `D<T>::x` naming the current instantiation `D` when lookup finds nothing and `D` has a dependent base ([temp.dep.type]/6; `Sema::unknown_specialization_nns` rebuilds the specifier on the injected-class-name type, so instantiation looks `x` up in the specialization, an implicit member access in a member function; WP-309; the same for a nested-name-specifier component `D::N::` when `N` is found neither as a nested name nor as an ordinary name in `D`, e.g. `using G::F::F;` with a dependent base `F<U, T, int>`, WP-367); also `B::x` when `B` is a member class of the current instantiation named outside its own definition that is declared but not yet defined (`struct Block;` defined out of line after the member function naming `Block::get`), the specifier being dependent (clang `computeDeclContext` has no context for it), so `x` is looked up in the instantiated member class (WP-726, `temp.dep.type/incomplete-member-class-qualifier{,-missing}.cpp`) | `DependentScopeDeclRefExpr {nns, name (a `DeclName`), targs?}` |
| `e.m` or `p->m` with a type-dependent object, or a member of the current instantiation that is not found when it has dependent bases | `CXXDependentScopeMemberExpr {base, arrow, nns?, name, targs?}` |
| unqualified `f(args)` with type-dependent args, where lookup finds functions or templates (or nothing) | `UnresolvedLookupExpr {requires_adl, overloaded, decls, targs?}`, plus `CallExpr` of dependent type |
| unqualified `operator@(args)` / `operator T(args)` (an operator- or conversion-function-id as the callee) whose lookup finds nothing ([temp.dep.general]/2, [basic.lookup.argdep]/1) | an empty `UnresolvedLookupExpr` with `requires_adl`, as for an undeclared `f(args)` (`Sema::act_on_id_expression_dn` while `deferring_call_ref`); ADL at the call or at instantiation, and no candidate is clang's `use of undeclared 'operator@'` (`sema_undeclared_use`) |
| `obj.f(args)` that names an overload set of members | `UnresolvedMemberExpr` |
| `a op b` with a type-dependent operand | `CXXOperatorCallExpr` whose callee is an `UnresolvedLookupExpr` holding the operator functions found by unqualified lookup at definition ([temp.dep.candidate]); if none are found, a `BinaryOperator`/`UnaryOperator` of dependent type (clang) |
| `typename T::type` | `DependentName` type (canonical keyword `typename`, types.md §18) |
| `class-key T::X` (`struct`/`class`/`union`/`enum`) with a dependent qualifier, as a reference or in `friend class-key T::X;` ([dcl.type.elab]/1-3, [class.friend]/3; WP-332) | `DependentName` type with the class-key as its keyword (parser `parse_tag_specifier`, clang `ActOnDependentTag`); with `friend`, a friend type. Instantiation (`dependent_tag_name`) looks up a tag of the substituted class: none gives `no struct named 'X' in 'Q'`, a tag of the other kind `use of 'X' with tag type that does not match previous declaration`. Gap: a found typedef reports the first (clang: `type alias 'X' cannot be referenced with the 'struct' specifier`) |
| `typename T::template X<U>` | `DependentTemplateSpecialization` type |
| `X<T>` where `X` is a class template | dependent `TemplateSpecialization` type |
| `T(args)` / `T{args}` with dependent `T` | `CXXUnresolvedConstructExpr` (list flag) |
| `int x(e)` with a type-dependent `e` | the initializer is kept as `ParenListExpr` |
| `T x(e)`, `T x = e` with a type-dependent `e` whose type is already a class `U` (`U(a)` with `a` type-dependent, a `CXXUnresolvedConstructExpr`) and a non-dependent `T` | `e` is kept unconverted and the initialization runs at instantiation (clang `InitializationSequence`'s `DependentSequence`), so `T x(U(a))` is direct-initialization there and an explicit `T(const U&)` is a candidate (WP-787, `temp.dep.expr/direct-init-from-dependent-functional-cast-explicit-ctor.cpp`) |
| a braced argument `{…}` inside a dependent `T(args)`, `new T(args)`, `x(args)` initializer or mem-initializer | kept as a dependent `InitListExpr` with its nested lists intact (`Sema::dependent_args`), not the flattened braced-argument placeholder (`temp.dep.expr/braced-arg-nested-list.cpp`, WP-418). A mem-initializer pattern is rebuilt at instantiation by `written_args` (WP-445) |
| `X x(e)`, `X x = e`, `X x{e}` whose type `X` is a dependent class type (including the current instantiation, [temp.dep.type]/7) | the initializer is kept unconverted (a `ParenListExpr` for the parenthesized form) and initialization runs only at instantiation, as clang's `AddInitializerToDecl`; overload resolution against a dependent target would treat every candidate's conversion as an identity and report ambiguity (WP-276) |
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
- Of a special member template by signature ([temp.expl.spec]/1,
  [temp.deduct.decl], WP-668): an out-of-line constructor,
  conversion-function or `operator=` declarator after `template<>`
  (`template<> P::P(const J&) : P(1) {}`) that matches no non-template
  member names the specialization of one of the class's constructor
  templates (every conversion function template, or the `operator=`
  templates) deduced against the declarator's type, chosen as for an
  explicit instantiation by signature (below), and is a new declaration
  of it whose body follows (`Sema::act_on_explicit_special_template_spec`).
  No match is "no function template matches function template
  specialization 'P'" with the failed-deduction notes
  (`temp.expl.spec/special-member-template-spec-run.cpp`,
  `special-member-template-spec-no-match.cpp`).

**Partial specialization.** A non-empty template-head whose
class-head-name (or variable declarator-id) is a template-id:
`template<class T> struct A<T*>`, `template<class T> constexpr bool v<T*>`.

**Explicit instantiation.**
- A definition, `template` followed by a declaration with no `<`:
  `template struct A<int>;`, `template void f<int>(int);`,
  `template void f(int);` (the args are deduced),
  `template int A<int>::s;`.
- A declaration: `extern template …`.
- By signature ([temp.explicit]/4, [temp.deduct.decl], WP-580): a
  function declarator with no template-id names the specialization of a
  function template found by redeclaration lookup of its name whose
  deduction against the declarator's type succeeds and gives that type
  (exception specifications aside); a defaulted template parameter takes
  its default. A non-template member of a class template specialization
  that matches is preferred (it is a member explicit instantiation). For
  an out-of-line constructor or conversion-function declarator that
  matches no non-template member, the candidates are the class's
  constructor templates, or every conversion function template of the
  class ([temp.mem]/6: a conversion template specialization is not found
  by name lookup), `Sema::act_on_explicit_special_template_inst`. Of
  several matches the most specialized is kept ([temp.func.order]).
  No match is "explicit instantiation of 'A' does not refer to a function
  template, variable template, member function, member class, or static
  data member", with a "candidate template ignored: …" note at each
  candidate's most recent declaration (none for conversion templates,
  whose failed candidates lookup never finds). Two matches neither more
  specialized than the other is "partial ordering for explicit
  instantiation of 'B' is ambiguous" with "explicit instantiation
  candidate function 'B::B<int *, int>' template here [with T = int *,
  U = int]" per candidate, in declaration order (clang
  `err_explicit_instantiation_ambiguous`). The specialization's kind then
  moves as for a class (table below), and its emission follows:

  | Form | Emitted (clang -O0) |
  |---|---|
  | `template R f(params);` (definition) | defined `weak_odr` in a comdat (structors: every variant, the `C5`/`D5` comdat) |
  | `extern template R f(params);`, not inline | not instantiated; a use is an external `declare` |
  | `extern template`, inline / constexpr | definition instantiated for constant evaluation; at -O0 a use is still a `declare`, at -O1 `available_externally` |
  | `extern template` then `template` | as the definition |

  An explicit instantiation declaration alone therefore needs another
  translation unit's explicit instantiation definition to link, as with
  clang.

**One declarator.** [temp.pre]/5: the declaration of a template head,
`template<>` or an explicit instantiation has at most one declarator.
The parser (`Parser::template_decl_kind`, taken by the next simple or
member declaration) reports clang's `err_multiple_template_declarators`
at the `,` after the first declarator ("a template declaration can only
declare a single entity", "an explicit template specialization can only
declare a single entity", "an explicit template instantiation can only
instantiate a single entity"), once per group. As in clang's
`ParseDeclGroup`, the later declarators are still declared, each as the
first one was: `Parser::template_group` saves the template-declaration
state before the first declarator (`Sema::save_template_decl_state`:
the open heads and the pending `template<>` / `[extern] template`
markers) and `Parser::next_declarator` restores it after each `,`
(`Sema::restore_template_decl_state`). So after
`template<class T> int a, b;` both are variable templates (`b` without
arguments is "use of variable template 'b' requires template
arguments"), and `template<class T> void f(), g();` declares a function
template `g` (WP-379, WP-392;
`tests/conformance/temp.pre/multiple-declarators.cpp`).

**Explicit instantiation with a definition** ([temp.explicit]/3, WP-392;
clang `ParseDeclarationAfterDeclaratorAndAttributes`,
`ParseFunctionDefinition`, `ParseClassSpecifier`). An explicit
instantiation is not a definition:
- a class whose class-head-name is a template-id followed by `{`
  (`template struct S<int> { … };`, also `extern template`) is
  "explicit template instantiation cannot have a definition; if this
  definition is meant to be an explicit specialization, add '<>' after
  the 'template' keyword" at the name, with a `<>` fix-it, and is
  defined as an explicit specialization;
- a function or variable declarator whose declarator-id is a template-id
  followed by a body, `= delete`, an initializer or a `,`
  (`Sema::act_explicit_inst_with_definition`) is the same error at the
  name and recovers as an explicit specialization (`template<>`), so a
  `static_assert` sees the body's value;
- with a plain declarator-id (`template constexpr int h() { … }`), it is
  "function/variable cannot be defined in an explicit instantiation; if
  this declaration is meant to be a function/variable definition, remove
  the 'template' keyword" at the `{`/`=`/`,`, and the `template` is
  ignored (a plain declaration or definition follows).
Test `tests/conformance/temp.explicit/instantiation-with-definition.cpp`.
After a template-id declarator-id a `(` always starts a parameter clause
(clang `Declarator::mayBeFollowedByCXXDirectInit` is false for a
non-identifier name; parse.md §2): `template int t<char>(1);` and
`template<> int t<char>(1);` are function declarators. That gives
"expected parameter declarator" and "expected ')'" at the `1`, then
"explicit instantiation of 't' does not refer to a function template,
…" or "no function template matches function template specialization
't'" (WP-406). Test `temp.explicit/template-id-paren-is-function.cpp`.

**Friends.** `template<class U> friend struct B;`,
`friend void f<>(T);` (a friend specialization), and
`template<class U> friend void g(U);`.

**Deduction guide.** At namespace scope, a declarator whose name is a
class template visible in scope, followed by a balanced `( … )` and then
`->`. The parser detects it with a token scan, without tentative sema
actions, and only when the decl-specifier-seq has no type specifier
(`Declarator::typed`; clang's `AllowDeductionGuide`): `auto mk(F) -> T&`
declares a function named like a visible class template `mk` (WP-789,
`temp.deduct.guide/typed-function-named-like-class-template-run.cpp`). It parses `[explicit-specifier] template-name (params) ->
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
- A builtin type-trait call (`__is_trivially_destructible(T)`, any
  `BuiltinKw::Trait`) is such a primary expression, as in clang's
  `ParseCastExpression(PrimaryExprOnly)`; libc++'s `<expected>` relies on
  it (WP-276).
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
- In an out-of-line member definition the explicit heads that match the
  qualifier's class-template levels (`match_template_lists_to_scope`)
  belong to the classes, so the first invented parameter creates the
  member's own list (or extends an explicit member head):
  `template<class T> void A<T>::f(C auto)` is a template of depth 1.
  `act_on_finish_template_decl` pops such invented heads (WP-169,
  `temp.mem/out-of-line-abbreviated.cpp`).
- `C auto` attaches the type-constraint `C<auto:N>`.
- When the first invented parameter opens a new head, the named
  parameters of the same clause declared before it (`int c` in
  `f(int c, auto& x)`) get `DEPENDENT_CTX` (`DeclArena::mark_dependent_param`):
  they are parameters of the template, and instantiation maps their uses to
  the specialization's parameters ([temp.inst]/3; WP-298, libc++
  formatter.handle.pass.cpp; `dcl.fct/abbreviated-leading-param-run.cpp`,
  `tests/codegen/abbreviated_template_leading_param.cpp`).
- `act_on_end_abbreviated` ends only an invented head opened by the
  declaration that just ended. In a member declaration of a class, the
  heads that were already open at the class's `{` are not popped
  (`ClassBuild::heads`). One of them is the head of a generic lambda
  that encloses a local class, and it stays open until the lambda ends,
  so the locals declared after the class are still declared in the call
  operator template ([expr.prim.lambda.general]/6; WP-789,
  `class.local/generic-lambda-local-class-member-then-local-run.cpp`).
- A friend function declarator finishes its invented head like any
  other declarator: `friend R f(S, C auto)` (defined in the class or not)
  declares an abbreviated friend function *template* ([dcl.fct]/22,
  [temp.friend]/1), not a function with a dependent parameter type. A
  call found by ADL then deduces it, so an atomic constraint calling it
  is non-dependent after substitution (WP-307, libc++
  `sized_sentinel_for` with range.iter.ops `distance_apriori_sentinel`;
  `temp.constr.atomic/abbreviated-friend-template.cpp`).
- Generic lambdas use the same routine (WP-075).
- Placement (WP-305, clang's `FunctionDeclarationScope` and
  `err_auto_not_allowed`; `Parser::parse_param_clause`). Only the clause
  of a function declaration invents: a declarator in a `File`, `Member`,
  `Block` or `ForInit` context, not of a `typedef`
  (`Declarator::typedef`), whose function declarator applies directly to
  the declarator-id (no chunk before it, grouping parentheses aside;
  `isFunctionDeclaratorAFunctionDeclaration`), or a lambda's clause. An
  `auto` in any other clause (a nested function declarator, a function
  type in a type-id, alias, typedef, variable or template parameter) is
  "'auto' not allowed in function prototype that is not a function
  declaration" at the `auto` (`sema_auto_not_allowed`, §6.3), and the
  parameter's type becomes `int`. `decltype(auto)` is never a parameter's
  placeholder ("... in function prototype", "... in lambda parameter").
- A parameter declarator with a trailing return type
  (`auto (*cb)(int) -> int`) keeps its decl-specifier `auto`: it is the
  function type's `auto` ([dcl.fct]/2; a `C auto` or `const auto` there is
  `sema_trailing_return_without_auto`). A placeholder in that trailing
  return type (under pointer, reference and further function
  declarators: `auto (*cb)(int) -> auto (*)(int) -> C auto`) is checked as
  above, reported at the decl-specifier, and otherwise invents a
  parameter (`void f(auto (*p)() -> C auto)` is a template; clang's
  `GetFullTypeForDeclarator`). Tests: `dcl.fct/p22-auto-nested-prototype*.cpp`,
  `dcl.fct/p22-auto-trailing-return-run.cpp`,
  `dcl.fct/p2-trailing-param-auto-diags.cpp`,
  `dcl.spec.auto/p2-decltype-auto-param.cpp`,
  `tests/mangle/abbreviated_trailing_auto.cpp`.

### 2.7 Pack expansions by context ([temp.variadic]/5)

| Context | Example | Parser site | M3 |
|---|---|---|---|
| function parameter pack | `void f(T... t)` | declarator | yes |
| template parameter pack that is a pack expansion | `template<T... v>` | §2.1 | yes |
| initializer-list / call arguments | `f(a...)`, `{a...}`, `T(a...)` | expression-list | yes |
| braced-init-list pattern ([temp.variadic]/5.4, WP-332) | `{{t, 1}...}`, `f({t, 1}...)`, `: m{{t, 1}...}`, `{.m = {t}...}` | `parse_init_list` / expression-list: `act_pack_expansion_list` keeps the list unanalyzed (an `InitListExpr` of type `void`) under a `PackExpansionExpr`; instantiation (`init_items`, `written_list`, `call_arg_list`) rebuilds one braced-init-list (or braced argument) per element | yes |
| base-specifier-list | `struct D : B<T>... {}` | base clause (WP-065) | yes |
| mem-initializer-list | `D() : B<T>(0)... {}` | ctor-initializer (WP-065) | yes |
| template-argument-list | `X<T...>` | §2.2 | yes |
| using-declarator-list | `using B<T>::f...;` | using-decl (WP-062) | yes |
| alignment-specifier | `alignas(T...)`, `alignas(N...)` | `parse_alignas`; one `ALIGNED` per element at instantiation (spec/sema.md §4.5, WP-302) | yes |
| lambda capture | `[t...]`, `[...x = t]` | WP-075 | yes |
| `sizeof...` | `sizeof...(T)` | primary expression | yes |
| fold-expression | `(t + ...)` | parenthesized primary | yes |
| pack-index | `t...[0]`, `T...[0]` | postfix and type | yes |
| attribute-list | `[[a...]]` | — | rejected, as clang |
| structured-binding pack | `auto [...xs] = e` | decomposition declarator (P1061, §7.1) | yes |
| friend pack | `friend T...;` | friend-type-specifier-list (P2893, §7.1) | yes |

A pattern that names no unexpanded pack is "pack expansion does not
contain any unexpanded parameter packs" at the `...`
(`sema_pack_expansion_without_packs`), except for a function parameter
pack ([dcl.fct]/16): there clang's wording names the parameter's declared
type before array/function adjustment, "type 'int' of function parameter
pack does not contain any unexpanded parameter packs"
(`sema_param_pack_without_packs`, `act_on_param_declarator`), and the
parameter is kept as a plain parameter. A misplaced `auto` in its clause
has already become `int` (§2.6), so `void f(auto (*...x)(C auto...))`
reports both. Test: `temp.variadic/p5-param-pack-without-packs.cpp`.
When the pack's declared type is erroneous (an unknown type name, a bad
member pointer, a reference to a reference) its declarator recovers as
clang's `GetFullTypeForDeclarator` does (`Sema::declarator_int_recovery`,
WP-390): a failed base or chunk becomes `int`, a function returning an
array or function returns `int`, a reference to a reference collapses.
The pack check then runs on the recovered type and reports it ("type
'int' of function parameter pack ..."), with no further errors. Tests:
`dcl.fct/pack-erroneous-type-recovers-to-int.cpp`,
`dcl.fct/pack-grouped-declarator-id.cpp`.

A function parameter pack with a default argument is "parameter pack
cannot have a default argument" at the `=` ([dcl.fct.default]/3,
`reject_param_pack_default`, from `act_on_param_default_arg` and the
in-place default path); the argument is discarded. A template parameter
pack's default ([temp.param]/14, `template<class... T = int>`, also
non-type and template template packs) is "template parameter pack
cannot have a default argument" at the `=`
(`act_on_template_param_default`). Tests:
`dcl.fct.default/pack-default-argument.cpp`,
`temp.param/pack-default-argument.cpp`.

A function parameter pack whose array bound alone names the pack
(`int ...a[Ns]`, or `int a[Ns]...` after the misplaced-ellipsis
recovery, spec/parse.md) expands the unadjusted array type; each element
decays when the pack is substituted, as in clang (WP-385; test
`temp.variadic/p5-array-bound-param-pack.cpp`).

A lambda whose call operator's type names an unexpanded pack (a
parameter `Ts a` whose `...` was misplaced or omitted) contains that
pack. Its enclosing initializer then gets "initializer contains
unexpanded parameter pack 'Ts'" ("expression contains ..." for an
expression statement; a pack expansion around the lambda expands it;
`act_on_lambda_declarator` marks the
lambda; `expr_packs` reads the call operator's type). Test:
`temp.variadic/p5-lambda-param-unexpanded.cpp` (WP-385).

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
| `TemplateSpecialization` | the name is dependent, or any arg is dependent; an alias template's: its aliased (canonical) type is | any arg is dependent or instantiation-dependent |
| `DependentName`, `DependentTemplateSpecialization`, `InjectedClassName` | always | — |
| `Decltype(e)` | `e` is type-dependent, which gives `DependentDecltype` | `e` is instantiation-dependent |
| array, pointer, reference, function, member pointer | any component is dependent; an array bound is value-dependent | any component |
| `Record`/`Enum` | the decl is in a dependent context | — |
| `PackExpansion` | its pattern is dependent | — (and UNEXPANDED_PACK is cleared) |
| `Auto` | undeduced inside a dependent context | — |

A reference to a variable or parameter with automatic storage declared
in a template is instantiation-dependent only so that instantiation
rebinds it (`decl_deps`). The `decltype` of such a reference (or of the
parenthesized reference) whose declared type is not dependent is the
plain `Decltype`, as clang computes it: `decltype(V)::iterator` with
`std::vector<int> V` names a member of a known class, and is a type in a
template argument with no `typename` (WP-730,
`tests/conformance/dcl.type.decltype/template-local-nested-name-run.cpp`).

**Three predicates (WP-445).** Every "is this dependent?" question
asks exactly one of:
- *dependent type* (`Sema::is_dependent_type`, the `DEPENDENT` bit,
  [temp.dep.type]/7): the canonical type is unknown until instantiation.
  Every semantic check on a type (completeness, conversions, base
  specifiers, `sizeof`, literal-type and trait checks, deduction of a
  parameter, switch/range-`for` conditions) waits only for this. A type
  that is merely instantiation-dependent (`void_t<T>` = `void`,
  `TC<N>*` with `N` a constant local, an alias whose aliased type ignores
  its arguments) is checked when parsed, as clang does.
- *instantiation-dependent* (`Sema::is_instantiation_dependent_type`,
  either bit): the type mentions a template parameter or a template's
  local, so substitution must still visit it and its sugar. Only the
  bookkeeping for a re-substituted pattern asks it: written locations
  (§5.8, `note_written_loc`, `begin_loc`, `note_ref_loc`,
  `type_spec_locs`, a trailing `->`), a declared type kept for the next
  substitution (`subst/decls.rs`), and the non-type template parameter
  types `ttarg*.rs` substitutes.
- *value-dependent* (`Sema::expr_is_value_dependent`, [temp.dep.constexpr]):
  an expression's value is unknown; constant evaluation, narrowing and
  `static_assert` wait for it. A value-dependent template argument makes
  its specialization a dependent type. A non-type template argument that
  is only instantiation-dependent is also kept as written and not folded
  (`is_same<void, void_t<decltype(e)>>::value`, whose qualifier is
  instantiation-dependent, as a `DeclRefExpr`'s qualifier makes it):
  substituting it again is what makes a failure in `e` a deduction
  failure ([temp.deduct.general]/8). The dependence may sit only in an
  operand type: `sizeof(void_t<decltype(e)>*)`, `alignof(...)` and a
  type trait's operands (`__is_same(first_t<int, decltype(e)>, int)`)
  count (`args.rs` `type_instantiation_dependent`). Such an argument's
  *canonical* form is still its converted value, as clang's is:
  `enable_if<sizeof(first_t<int, decltype(T::x)>) == 4>` names the
  specialization `enable_if<true>` (its `::type` is found at the
  definition), while the written argument is what a partial
  specialization's deduction substitutes again (WP-465). A default
  argument after it is substituted with that converted value, so in a
  member of a class template `AS<sizeof(buf)>` with `template<size_t L,
  size_t A = mx<L>::value>` names `AS<32, 16>`, not a specialization whose
  second argument is a dependent `mx<sizeof(buf)>::value` (libc++'s
  `aligned_storage` in `std::function`'s swap; WP-685,
  tests/conformance/temp.arg.general/default-after-instantiation-dependent-argument.cpp).

`sizeof...(U)` with `U` bound to a list holding pack expansions (an alias
template named `N<Ts...>`) is the value-dependent `k + sizeof...(P1) +
...` (`pack.rs` `size_of_expansions`), so `N<Ts...>` stays dependent.

### 3.2 Expression dependence (`ExprDeps`)

Dependence is computed once by each `AstBuilder` constructor, from its
operands' deps and its own type's flags (`deps_of_type_flags`). It is
never recomputed (ast.md §5). The rules follow [temp.dep.expr],
[temp.dep.constexpr] and clang's `ComputeDependence.cpp`.

| Expression | deps |
|---|---|
| any expression | the union of its children's deps, plus `deps_of_type_flags(type)` (gives TYPE when the type is dependent) |
| `DeclRefExpr` to an NTTP | VALUE \| INST (plus TYPE if the parameter's type is dependent; plus UNEXPANDED_PACK if it is a pack) |
| `DeclRefExpr` to a variable | TYPE if its type is dependent; VALUE if it is a const integral or `constexpr` variable whose initializer is value-dependent (ast.md §16.15); TYPE \| VALUE \| INST if it is a static data member of the current instantiation declared in its class as an array of unknown bound without an initializer there ([temp.dep.expr]/3, `static const Info Ops[];`). `sizeof`/`alignof` of such a type-dependent operand is checked at instantiation, where a (parenthesized) reference to a static data member of a specialization whose type is an array of unknown bound first instantiates the member's definition and names it with the bound its definition gives (clang `completeExprArrayBound`; [temp.inst]/4; WP-730, test `temp.dep.expr/static-member-array-unknown-bound.cpp`) |
| `this` in a dependent context | TYPE |
| `sizeof`, `alignof`, `noexcept`, `typeid(type)`, `__is_*` traits, `requires` | the operand's TYPE becomes VALUE (the result type is fixed); `alignof e` naming a variable or member whose `alignas` operand is dependent is VALUE-dependent too (`Builder::names_dependently_aligned_decl`, clang `computeDependence(UnaryExprOrTypeTraitExpr)`; WP-312) |
| `noexcept(e)` with a can-throw result that is dependent | also VALUE \| INST |
| `ConceptSpecializationExpr`, `RequiresExpr` | VALUE if any argument is dependent |
| `SizeOfPackExpr` | VALUE \| INST (UNEXPANDED_PACK is cleared) |
| `PackExpansionExpr` | its pattern's deps without UNEXPANDED_PACK |
| `CXXFoldExpr` | its operands' deps without UNEXPANDED_PACK (for the pack operand) |
| `UnresolvedLookupExpr` | TYPE \| VALUE \| INST if any explicit template arg is dependent; otherwise `requires_adl` decides at the `CallExpr` |
| a lambda in a dependent context | INST |
| a named or C-style cast to a dependent type (`dependent_cast`) | its type's deps; the value category is the target's, read through sugar (`static_cast<result_type>(b)` with `typedef X<R>& result_type;` is an lvalue, rebuilt as a reference cast; libstdc++ future:751; WP-332, test `expr.static.cast/libstdcxx-dependent-cast-to-reference-typedef.cpp`) |
| `CXXUnresolvedConstructExpr` (`T(args)`, `T{args}`) | its type's deps; an argument's TYPE becomes VALUE unless the type is an undeduced placeholder (`auto(x)`, CTAD), which stays TYPE-dependent ([temp.dep.expr]/3: `u16(t.v)` has type `u16`, so `RP d(u16(t.v));` is direct-initialized at definition; WP-775, `temp.dep.expr/functional-cast-not-type-dependent-run.cpp`; the placeholder case WP-782, `temp.dep.expr/p3-placeholder-functional-cast-type-dependent-run.cpp`) |
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
  has no dependent bases: "no member named 'x' in 'A<T>'". The class prints
  as its injected-class-name type, like clang's DeclContext printing
  (`Sema::current_instantiation_desc`, WP-315):
  - The pattern prints unqualified with its parameters' names: `V<T>`,
    and `P<T...>` for a pack.
  - A partial specialization prints with its canonical arguments:
    `V<type-parameter-0-0 *>`.
  - A non-template nested class prints qualified (`W::I`).
  - Test: tests/conformance/temp.dep.type/no-member-in-current-instantiation-name.cpp.
- A missing `typename` or `template` (§2.3), a redefined default
  argument, a malformed template head.

**Instantiation-dependent but not dependent** (WP-404). `TC<N>`, with
`N` a constant local of a function template, is instantiation-dependent
only (§3.1). Like clang (`anyDependentTemplateArguments` tests
`isDependent`), it does not defer a check:
- A concept-id over it is satisfied at once. `concept_spec_expr` is
  value-dependent only when an argument is dependent; an
  instantiation-dependent argument leaves it `INSTANTIATION` only.
- A type trait over it (`__is_object(TC<N> *)`) is evaluated on the
  argument's canonical type, so a variable template specialization
  instantiated with it (`is_object_v<T *>`) does not cache an
  unevaluated `false`.
- A function or variable template specialization keyed by canonical
  arguments stores the canonical form of such an argument
  (`Sema::settle_inst_sugar`; clang builds a specialization from its
  canonical converted arguments). Another function template's use of the
  same specialization (an implicit deduction guide's, `subrange(p, p)`)
  must never substitute the first template's local.
- Test: tests/conformance/temp.dep.expr/instantiation-dependent-arguments.cpp.

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

*As built (WP-070a; measured in WP-106, §14 Q3).* Sharing is
unconditional. No `LOCAL` bit was added: the builder marks references to
decls of a dependent context `INSTANTIATION`, so the test is
`deps & (TYPE_VALUE_INST | UNEXPANDED_PACK) == 0`. The flag does not
exist. On `libcxx_m3.cpp`, sharing saves 0.8% of `inst.bytes` (at most
3%).
- A shared subtree is still odr-used by each instantiation, as
  clang's rebuilt nodes are: `mark_shared_expr_referenced`
  (`templ/subst/shared.rs`) walks it and marks the functions and
  variables it names referenced. The walk includes the expressions in a
  name's explicit template arguments (`call<&H::c>(*this)`). A
  non-dependent expression template argument that the substitution keeps
  (`Subst::template_arg`) is walked as well. Before WP-690 the member
  function in `&H::c` was never instantiated when the argument sat in a
  generic lambda in a member of a class template, so the link failed
  (test `temp.arg.nontype/kept-argument-odr-use.cpp`).
- The expected saving is 30–60% of the expression nodes in libc++ bodies.
  It is measured by the instantiation report (§11.3).

### 3.4 Current instantiation ([temp.dep.type]/1)

These are the current instantiation:
- inside a class template (or a partial specialization), the
  injected-class-name;
- `A<T>` or `A<T...>` written with the class's own parameters in order;
- the nested classes of either.

A template-id of the class whose pack expansion argument falls on a
parameter that is not a pack (`Ts<T...>` in `template<template<class>
class H, template<class> class... T> struct Ts`) names neither the
current instantiation nor a partial specialization's injected class: it
is a dependent specialization checked once substituted (WP-730,
`temp.local/injected-name-pack-expansion-args.cpp`).

A template-id of a partial specialization's template whose non-type
arguments are written unconverted is matched by converting them first
(`entered_partial_by_converted_args`). One whose primary template has a
template parameter pack is converted first too: the partial
specialization's own arguments hold the arguments for the pack as one
argument pack, so `template<class... A> int B<int, A...>::g()` is found
only after conversion (WP-726,
`tests/conformance/temp.spec.partial.member/out-of-line-member-pack-arguments.cpp`).

Qualified lookup into the current instantiation happens at definition.
Lookup of a found member binds it (`typename A<T>::X` resolves to the
member typedef, not to `DependentName`). If the member is not found and
the class has dependent bases, the result is a dependent node.

Inside a member of a class whose definition has a dependent base, an
id-expression that names a non-static member (or an overload set or
member template containing one) of a class not known to be a base of
`*this` is an implicit `this->Q::name`. It is built as a dependent member
access on an implicit `this` object (`Sema::dependent_implicit_member`)
and resolved at instantiation (`Sema::member_via_dependent_base`; clang
`IMA_Dependent`); when `Q` turns out not to be a base, the error is
`'Q::name' is not a member of class 'D<O>'`.
libc++ `basic_ostream::operator<<(int)` calls `ios_base::flags()` through
the virtual base `basic_ios<C>` (WP-201).

Destructor names in a member access whose object type is dependent
([expr.prim.id.dtor]/2, [basic.lookup.qual]/5, [temp.dep.expr]/3;
WP-457): `p->~T()`, `x.T::~T()`, an alias `~U` and an injected-class-name
resolve at definition as before; a name the definition context does not
find (`b->~Base()` naming a class template, `b->~Alias()` naming a member
of the object's class) is kept as a `CXXPseudoDestructorExpr` with a
dependent destroyed type and the identifier in the side table
`TemplateTables::pseudo_dtor_name` (`Sema::dependent_destructor_name`).
Substitution (`act_on_member_destructor_late`) looks the name up in the
instantiated object's class only — not found there, or a non-class
object, is `undeclared identifier 'N' in destructor name` — and then
builds the ordinary destructor call (or pseudo-destructor). A dependent
nested-name-specifier ending in the destructor's class name
(`p->Base<T>::~Base()`) is a qualified pseudo-destructor of the
qualifier's type, checked against the object at instantiation, and
`x.~decltype(e)()` is a pseudo-destructor of `decltype(e)`'s type
(parse.md). Tests:
`tests/conformance/expr.prim.id.dtor/dependent-member-destructor-name*.cpp`.

A member access the pattern resolved into the current instantiation
(`l.base()` with `l` a `const C&`, in a friend's trailing `decltype`
inside `C`'s definition) is rebuilt at instantiation without a
completeness check while that specialization is being defined (clang
`TreeTransform::RebuildMemberExpr` does not call `RequireCompleteType`;
`Sema::pattern_member_access`, set by `subst/exprs.rs` for the rebuild
and read once by `member_object`): libstdc++ 16 `__normal_iterator`'s
`friend auto operator-(...) -> decltype(l.base() - r.base())` (WP-525,
`temp.inst/friend-trailing-decltype-current-instantiation.cpp`).

### 3.5 ADL and operator lookup at instantiation ([temp.dep.candidate])

When a dependent unqualified call or operator is substituted:
1. The candidates are the decls stored in the `UnresolvedLookupExpr`
   (found by unqualified lookup at definition).
2. Then ADL runs on the *instantiated* argument types, against the
   **current** state of the namespace tables.
3. For operators, the built-in candidates are added (S3O).

*Implementation (WP-207).* `Substituter::callee` keeps the stored decls
when every one is a namespace-scope function or function template (through
`using` shadows, linkage specifications and `export`): it rebuilds the `UnresolvedLookupExpr` from
them with `requires_adl` set (and the substituted explicit template
arguments, if any), and the call resolves against them plus ADL.
It does not look the name up again from the scope current at
instantiation. That scope can differ from the definition's: constraint
satisfaction (§8) substitutes outside the instantiation context, so a
re-lookup from, say, namespace `std` found `std::swap` instead of the
deleted `ranges::__swap::swap` poison pill, or the `ranges::begin`
object instead of the deleted `ranges::__begin::begin`. A partial
specialization's arguments are substituted where the specialization is
named: an unqualified template-id call there (`declval<const T &>()` in
doctest's `is_std_string`) found nothing when named outside the
template's namespace, so the partial specialization never matched
(WP-683, `tests/conformance/temp.dep.candidate/template-id-definition-lookup.cpp`).
Other stored sets (class members, block-scope declarations) still take
the earlier path.

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
    explicit_member_insts: HashMap<DeclId, Vec<DeclId>>,  // member explicit instantiation
        // definitions (`template void X<int>::f();`), keyed by the decl they follow in their
        // lexical context (the context itself when it was empty); codegen emits them there
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

1. `check_template_args(tmpl, written)` ([temp.arg]), with the head
   declaration's parameters (§4.8.1):
   - matches kinds and arity;
   - fills missing args from defaults, substituting the earlier args into
     them (frame `DefaultTemplateArgumentInstantiation`). A dependent
     default is substituted in the template's semantic context
     (`in_inst_context(ctx of tmpl)`, as clang's
     `SubstDefaultTemplateArgument` enters `Template->getDeclContext()`),
     not the scopes of the use: a name the substitution looks up again
     (an unqualified call with explicit template arguments,
     `declval<Ts>()`) resolves from the template's namespace or class
     ([temp.dep.candidate]/1; WP-378,
     `tests/conformance/temp.param/default-argument-lookup-context.cpp`).
     A template in a local scope keeps the use's scopes (its function's
     names live only there). The same rule (`Sema::in_template_context`)
     covers the type of a non-type parameter substituted with the
     arguments before it (`template <class T, decltype(mk<T>()) N>`) and,
     for a function template, the explicit arguments' substitution into
     the function parameter types (§ "Landed (WP-071)"; WP-387);
     The substituted default (dependent or not) is then checked under a
     `DefaultTemplateArgumentChecking` frame, "while checking a default
     template argument used here" at the template-id's `>` (clang
     `CheckTemplateArgumentList`; the substitution frame has ended). The
     parser hands the `>` over with the argument locations
     (`set_template_arg_locs`); a template-id formed by substitution
     points at its name instead (clang: its `>`; follow-up). Defaults
     used by deduction are checked inside their substitution frame
     (`tests/conformance/temp.arg.general/default-argument-checking-and-narrowing.cpp`);
   - converts NTTP args (converted constant expressions, evaluated by the
     Oven with `EvalKind::TemplateArg`). A class-type argument converted
     by a user-defined conversion narrows through the standard conversion
     after it: the conversion function's result is folded and checked
     against the parameter type (`Z<CI{}>` with `operator int` to `bool`,
     "evaluates to 2, which cannot be narrowed"; clang `getNarrowingKind`
     on `ICS.UserDefined.After`). A braced-init-list argument (P2308,
     a DR) copy-list-initializes the parameter
     (`act_on_braced_template_arg`): narrowing is an error, a class type
     is aggregate- or constructor-initialized, a placeholder type is
     deduced from the list (CTAD for a class template; plain `auto` is
     "cannot deduce type of initializer list"), and a dependent list is
     kept until substitution. A braced default argument of a non-type
     parameter follows the same rule;
   - packs the trailing args into a `Pack`.
   It returns `ConvertedArgs { sugared: ArgListId, canonical: ArgListId }`.
2. Probe `specializations[(first(tmpl), canonical)]`. On a hit, return
   that decl.
3. On a miss, create a `ClassTemplateSpecialization` decl with
   `SpecInfo{kind: Undeclared, args: canonical}` and insert it. The
   implicit specialization's arguments are canonical (clang's
   `ClassTemplateSpecializationDecl`), so its members see `unsigned long`,
   not the alias `sz` it was first named with; the written spelling stays
   on the `TemplateSpecialization` sugar
   (`tests/conformance/temp.inst/canonical-arguments-in-instantiation.cpp`).
   - No instantiation happens. The type `X<int>` is
     `TemplateSpecialization(sugar) → Record(spec)`.
   - This is clang's `CheckTemplateIdType`.

With dependent args, steps 2–3 are skipped. The result is a dependent
`TemplateSpecialization` type, unless it names the current instantiation
(§3.4). Its arguments are not converted, so a non-dependent expression
argument (the `1` of `X<T, 1>`) carries its canonical profile and compares
as an expression ([temp.over.link]/5): two declarations writing `X<T, 1>`
name the same type and redeclare the same function template
(`tests/conformance/temp.over.link/non-type-literal-arg-in-dependent-specialization.cpp`).
Divergence: clang converts such arguments first, so `X<T, true>` and
`X<T, 1>` for a `bool` parameter are equivalent there and distinct here.

### 4.3 Triggers for implicit instantiation ([temp.inst]/2)

Kiln instantiates through exactly one hook,
`Sema::require_complete(ty, loc, diag) -> bool`, which is used wherever
sema.md and S3C require a complete type. Those places are:
- defining an object;
- member access and qualified lookup into `X<…>::`;
- `sizeof`/`alignof`;
- conversion to a base, and derived-to-base checks during overload
  resolution;
- deduction of a class template-id `P` against the bases of `A`
  ([temp.deduct.call]/4.3, §6, silently);
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
   - check each deduced template template argument against its
     parameter ([temp.arg.template]/3, §3.2 rules, in the SFINAE frame):
     a mismatch fails the deduction, as clang's
     `ConvertDeducedTemplateArgument` does, so `T<Ts...>` for
     `template<class...> class T` does not match `iseq<int>` of
     `template<class T, T...>` and cannot make `iseq<T, Is...>`
     ambiguous; a function template's deduced template template
     arguments are checked the same way after deduction, a failure being
     a substitution failure shown without the deduced arguments (WP-669,
     `tests/conformance/temp.arg.template/p3-deduced-argument-checked.cpp`);
   - finish the deduction: substitute the deduced args into `p.args` and
     require canonical equality with `A`;
   - check `p`'s associated constraints (§8). For a class template, their
     atoms check access from the specialization being matched
     (`ConceptState::partial_access`), so a class that befriends
     `traits<It>` or `traits` lets `requires It::flag` see its private
     members (clang accepts; WP-446, libc++ 22's
     `__product_iterator_traits`,
     `tests/conformance/temp.spec.partial.match/constraint-access-befriended-specialization.cpp`).
   The partials that succeed form the set `M`.
4. If `M` is empty, the pattern is the primary template. If `M` has one
   element, it is that partial. Otherwise the pattern is the most
   specialized partial, found by function-template partial ordering of
   the rewritten `template<p.params> void f(X<p.args>)` (§6.7), with
   constraints as the tie-breaker. In each direction a pack expansion
   among the synthesized arguments is deduced as a whole by a pack
   expansion of the other partial ([temp.deduct.type]/9,
   `DeducedArgs::expansions`), so `template <class... P> X<0, T<P...>>`
   is more specialized than `template <int R, class... P> X<R, T<P...>>`
   whatever the indices of the two `P`s (WP-292; test
   `templates/spec/temp.spec.partial.order-pack-expansion.cpp`). Each
   direction also finishes the deduction ([temp.deduct.partial]/12 via
   [temp.deduct.type]/1, clang `FinishTemplateArgumentDeduction`): the
   other partial's arguments with the deduced values substituted must
   equal the synthesized arguments, non-deduced contexts included, packs
   compared element-wise and nothing converted, so `X<M, when<c>>` is not
   at least as specialized as `X<T, when<f<T>::value>>` (WP-787,
   Boost.Hana; `partial_args_match` with `ordering`,
   `tests/conformance/temp.spec.partial.match/order-non-deduced-context-must-match.cpp`).
   A request for the specialization while its partials are matched
   (step 3 substituting a default template argument that names it, as
   `X<T, enable_if_t<is_convertible_v<T, S>>>` with a constructor
   template of `S` defaulting to `typename X<T>::type`) finds it
   incomplete, a substitution failure in that SFINAE context, as clang's
   `getPatternForClassTemplateSpecialization` does on
   `isAlreadyInstantiating` (`InstState::selecting`; WP-787,
   `tests/conformance/temp.inst/partial-match-recursive-request-incomplete.cpp`).
5. If no unique best partial exists, the error is "ambiguous partial
   specializations of 'X<int *>'", with a note "partial specialization
   matches [with T = int]" for each candidate.
6. Record the choice in `SpecInfo.from` and `SpecInfo.from_args` (the
   deduced args of the partial).

Explicit specializations never reach this step: they are found by the
probe in §4.2.

**In-class explicit specializations of member variable templates**
([temp.expl.spec]/3, CWG 727). `template<> constexpr bool v<int> =
true;` in a class declares a static data member without `static` being
written: its initializer is parsed in place, not saved as a default
member initializer. In a class template, each instantiation redeclares
it for the instantiated member template
(`Substituter::member_explicit_var_spec`): its arguments and type are
substituted, it becomes that template's explicit specialization, and its
initializer is instantiated at once. Codegen emits an odr-used one as it
does a member of the instantiated class, `linkonce_odr`
(`instantiated_var_def`), as clang (WP-789,
`temp.expl.spec/in-class-member-variable-template-spec-run.cpp`).

**Member class templates** ([temp.spec.partial.member]/2). When a class
template is instantiated, each in-class partial specialization of a
member class template is redeclared for the instantiated member template
(`Substituter::member_partial_spec`). Its template parameter list and
arguments are substituted with the enclosing level, so `In<T, B>` of
`Outer<int>` matches `In<int, char>`. The redeclaration records the
original partial as its pattern (`instantiated_from_map`). A
specialization that matches it is instantiated from the original
partial's definition, with the enclosing arguments as the outer levels
plus the deduced arguments (§4.6). An in-class partial specialization
of a member *variable* template is redeclared the same way
(`AstBuilder::var_partial_spec` with the substituted type), and its
type and initializer are instantiated from the original partial (the
`instantiated_from_map` entry; `instantiate_var_spec_decl`): libstdc++
16 `<optional>` `__not_constructing_bool_from_optional<_From, bool>`,
`<bits/hashtable.h>` `__is_key_type` (WP-525,
`temp.spec.partial.member/member-var-partial-spec-in-class-template.cpp`).
Its associated constraints stay the original partial's, unsubstituted
(`member_constraints`, as for a member template): matching checks them
with the enclosing specialization's levels plus the deduced arguments
(`template <forward_range T> struct cat<T>` in libc++'s
`lazy_split_view`; WP-276).

Member bodies of an in-class partial specialization of a member class
template ([temp.local]/8, [class.mem.general]/7; WP-332): the bodies,
default arguments and *noexcept-specifier*s are parsed after the
outermost enclosing class is complete, and the replay re-enters the
partial's own template parameter scope (`act_on_reenter_template_scopes`
takes a partial specialization's parameter list from its
`PartialSpecInfo`, as it takes a member class template's from its
template), so `I` in `template<class I> struct cmp<I, void> { int f()
{ return I(1); } };` names the parameter. libstdc++ stl_list.h:189,
chrono:2449, ranges:1888. Tests:
`temp.spec.partial.member/libstdcxx-member-partial-spec-params-in-body.cpp`,
`temp.spec.partial.member/libstdcxx-constrained-member-partial-spec.cpp`.

Out-of-line member template definitions ([temp.local]/7; WP-782): in
`template<unsigned size> int P::f()` the definition's own template
parameters (depth at or above the class's template levels) are found
before the members of `P` and its bases, although their scope lies
outside the re-entered class scope; the enclosing class template's own
parameters are still hidden by members (`own_template_param_outside`
in `kiln_sema/src/lookup.rs`; LLVM `AArch64InstPrinter.cpp`
`printGPRSeqPairsClassOperand<unsigned size>`). Test:
`temp.local/p7-member-template-param-not-hidden-out-of-line.cpp`.

Out-of-line members of a partial specialization whose argument list is
the primary's own (`template<class V> requires fw<V> class C<V>`;
[temp.spec.partial.member]/1, [temp.over.link]/6; WP-332): the qualifier
`C<V>::` in `template<class V> requires fw<V> template<bool B> class
C<V>::It` names the partial, not the primary, when the matching
enclosing template head's requires-clause is equivalent to the
partial's and not to the primary's (`current_instantiation` →
`own_args_partial`, only while entering a declarator context). Without
it the head was matched against the primary and diagnosed as "requires
clause differs". libstdc++ ranges:6380 (`chunk_view<_Vp>::_Iterator`).
Tests: `temp.spec.partial.member/libstdcxx-out-of-line-member-of-constrained-partial-same-args.cpp`,
`temp.mem.class/libstdcxx-constrained-partial-spec-member-def.cpp`.

Out-of-class partial specializations of a member template (WP-150):
`template<class T> template<class U> struct Outer<T>::Inner<U*> {...}`.
- The parser takes `Outer<T>::` as a tag qualifier and enters its scope
  for the body ([class.nest]/3); the qualifier consumes the outer head
  (`match_template_lists_to_scope`), the inner one heads the partial.
- The partial is created on the member template of the pattern with
  `Outer` as its semantic context and the namespace as its lexical one,
  so it is not in `Outer`'s member list. A definition of one declared in
  the class completes that declaration; `}` returns to the context the
  definition began in (`ClassBuild::outer`), not the lexical one.
- When the member template is redeclared in `Outer<X>`
  (`Substituter::member_template`), each partial of it not lexically in
  the class is redeclared as above (clang's `VisitClassTemplateDecl`).
  One declared after `Outer<X>` was instantiated is not part of it, as
  with clang.

### 4.5 Explicit specializations and instantiations

**Placement** ([temp.expl.spec]).
- An explicit specialization is allowed in any scope where the primary
  template may be defined. That includes class scope (CWG727).
- It must come before the first use that would cause implicit
  instantiation. Otherwise the error is "explicit specialization of
  'X<int>' after instantiation", with the note "implicit instantiation
  first required here" pointing to `SpecInfo.point_of_instantiation`.
- A class explicit specialization may name the primary with a
  nested-name-specifier (`template<> struct N::Z<int> {};`,
  [temp.expl.spec]/3): the qualifier is resolved, the primary looked up
  in the nominated scope, and the unqualified path reused. The enclosing
  namespace must contain the template's ("class template specialization
  of 'Z' not in a namespace enclosing 'N'", or "... must occur at global
  scope" for a global template, with "explicitly specialized declaration
  is here"); a name that is not a class
  template, a missing template-id, or an undeclared name take clang's
  errors (WP-409, `temp.expl.spec/qualified-class-spec{,-errors,-non-template}.cpp`,
  `temp.expl.spec/qualified-class-name-errors.cpp`).
- A qualifier naming a declared but undefined explicit specialization
  (`A<long>::B<int>::f` after `template<> template<> struct A<long>::B<int>;`)
  is "implicit instantiation of undefined template" / "incomplete type
  named in nested name specifier" with the template's or the
  specialization's note (`require_complete_nns`). At compat 22 the error is
  at the start of the whole qualifier, at 19 at the template-name; clang
  repeats it for a decl-specifier at namespace or class scope (`dup(clang)`,
  spec/diag.md §7.4.1). Tests: `temp.expl.spec/undefined-member-template-spec-qualifier.cpp`,
  `temp.expl.spec/undefined-spec-member-definition.cpp`,
  `temp.res/undefined-template-qualifier{,-clang19}.cpp`.
- A declarator qualifier that names a class template specialization
  matching no partial specialization (`template<class T> void P<T*, int>::f()`)
  is "nested name specifier 'P<T *, int>' for declaration does not refer
  into a class, class template or class template partial specialization"
  (`TemplState::unmatched_qualifier`); the declaration is dropped.
  An out-of-line member of a partial specialization that matches none of
  its members prints the specialization as written at compat 22
  (`'P<T, int>'`) and canonically at 19 (`'P<type-parameter-0-0, int>'`).
  Test: `temp.spec.partial.member/out-of-line-member-mismatch{,-clang19}.cpp`.

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
pattern and not yet explicitly specialized, including the out-of-line
definitions of static data members (clang `InstantiateVariableDefinition`)
and the members of member classes. Member templates are excluded, and
so is the partial specialization of a member class template the
instantiated class declares, which is still dependent (WP-812). A
member class that is not yet defined at that point gets the member
specialization kind `ExplicitInstantiationDefinition` (so its vtable and
RTTI are emitted `weak_odr`); one already defined keeps its implicit
kind (clang `InstantiateClassMembers`; `instantiate_members_of`). The
definition marks the class's vtable used, and that of each member class
it gives the definition kind, so their virtual members are odr-used and
an implicit virtual destructor is defined with it (clang
`MarkVTableUsed(..., true)`; WP-730,
`temp.explicit/vtable-implicit-virtual-destructor-run.cpp`). A
member explicit instantiation of a constructor or destructor
(`template X<int>::X();`) reaches sema through the special-member
declarator path and names the existing member (WP-170). An explicit
instantiation may not be `inline` ("explicit instantiation cannot be
'inline'"), nor `constexpr`/`consteval` when it names a function
("explicit instantiation cannot be 'constexpr'"); both are diagnosed and
the instantiation proceeds (clang `ActOnExplicitInstantiation`).

**An explicit instantiation definition of a function or static data
member whose template is not yet defined** (clang
`InstantiateFunctionDefinition`/`InstantiateVariableDefinition`) is
queued (§4.4) and instantiated at the end of the TU, so a later
definition is used and emitted. If the pattern is still undefined there,
the explicit instantiation is an error at its point of instantiation:
"explicit instantiation of undefined function template 'h'" for a
function template specialization (a member template's included), else
"explicit instantiation of undefined {member function|static data
member} 'f' of class template 'X<int>'". Either has the note "explicit
instantiation refers here" at the pattern declaration (WP-194). A
class explicit instantiation skips undefined members, and an explicit
instantiation declaration is never diagnosed.

The same holds for a variable template specialization (WP-214):
"explicit instantiation of undefined variable template 'N::v<int>'",
printed qualified and with the canonical arguments. It is reported once.
The location follows clang's point of instantiation.
- If the specialization was odr-used before the explicit instantiation,
  the error is at its first reference, even an unevaluated one
  (`Sema::note_var_referenced` records it in `inst.undefined_var_uses`).
- Otherwise the error is at the explicit instantiation.

A static data member that an earlier use already queued is queued again
by its explicit instantiation (`queue_explicit_instantiation`). It is
then reported at both places, as clang's duplicate
`PendingInstantiations` entries do.

**An explicit instantiation declaration** (`extern template`,
[temp.explicit]/12, WP-449) suppresses the implicit instantiation of the
definitions it covers, even when they are odr-used
(`Ast::governing_explicit_inst_kind` decides, as codegen does: the
member's own explicit kind, else the innermost explicitly instantiated
enclosing class):
- A member function's definition is instantiated only when its pattern
  is inline (declared `inline` or `constexpr`/`consteval` on a
  redeclaration, or defined in its class) or its declared return type
  contains a placeholder (clang `InstantiateFunctionDefinition`;
  `Sema::suppressed_by_explicit_inst_declaration`). The others are
  never instantiated here, so an ill-formed body is not diagnosed.
- A static data member's out-of-line definition is instantiated only
  when the member may be usable in constant expressions (clang
  `InstantiateVariableDefinition`, `might_be_constant_var`). It stays
  in `inst.member_defs`, so a later explicit instantiation definition
  still instantiates it.
- The declaration instantiates the definitions of the class's member
  classes, recursively, and gives the fresh ones the declaration kind
  (clang `InstantiateClassMembers` with
  `TSK_ExplicitInstantiationDeclaration`). A later explicit
  instantiation definition upgrades such a member class to a
  definition, so its vtable is required. A member class already
  implicitly instantiated keeps the implicit kind.
- An instantiated member function is implicitly inline only when its
  pattern is (`Sema::instantiated_pattern_inline`, clang copies
  `isInlined`). An out-of-class non-inline pattern gives a non-inline
  instantiation, with or without an explicit instantiation. Codegen
  reads the instantiation's own inline-ness (spec/codegen.md
  "Explicit instantiation declarations").

Tests: `tests/conformance/temp.explicit/extern-template-no-definitions.cpp`,
`extern-template-inline-error.cpp`, `implicit-inline-from-pattern-ast.cpp`,
`tests/codegen/extern_template_nested_class.cpp`,
`extern_template_optnone_o1.cpp`.

**Member specializations** such as `template<> void A<int>::f()`:
1. Instantiate the declaration of `A<int>` (declarations only).
2. Find the member `f` through `member_map`.
3. Set its `MemberSpecInfo.kind = ExplicitSpecialization` and attach the
   new definition.

**Member templates of an implicit instantiation** ([temp.expl.spec]/16),
such as `template<> template<class U> struct A<int>::B {...}` (also
partial specializations `A<int>::B<U*>`, and function and variable
member templates):
1. The parser sees `template<>` immediately followed by `template <`
   and brackets the inner declaration with
   `act_member_template_spec_start/end`. While `TemplState.member_spec`
   is set, the outer empty head does not count as a member-spec header.
2. The qualified lookup into `A<int>` finds the member template that
   `Substituter::member_template` redeclared there. The new declaration
   becomes a redeclaration of it, even when it is only a declaration
   (`template<> template<class U> struct A<int>::B;`).
3. `specialize_member_template` removes the `instantiated_from` link of
   the first declaration. `inst_template` and the pattern selection then
   stop at this declaration, like clang's `setMemberSpecialization`, so
   `A<int>::B<X>` instantiates the new definition.
4. If the name is not found in `A<int>`, the error is "out-of-line
   definition of 'N' does not match any declaration in 'A<int>'". This
   is the same text used for templated qualified tags in general (the
   scope is printed with written template parameter names, for example
   'A<T>').
5. "implicit instantiation of undefined template" notes "template is
   declared here" at the latest redeclaration of the pattern when no
   partial specialization matched, as clang does.
6. A partial specialization declared this way (`template<> template<char
   c> struct O<int, char>::A<1, c>`) has no enclosing template levels:
   the empty head adds no depth, so for a namespace-scope `O` its
   parameters are at depth 0, not at the member template's depth 1.
   Its specializations are instantiated with the enclosing levels cut to
   its own depth, then its deduced arguments (`partial_levels`, used by
   class instantiation and `spec_args`). Test:
   `temp.expl.spec/member-partial-spec-of-implicit-spec-run.cpp`.

**Constructors and destructors of an implicit instantiation**
(`template<> A<int>::A(int) {...}`, [temp.expl.spec]/1): the out-of-line
redeclaration marks the member specialization explicit
(`mark_member_explicit_spec`) and consumes the `template<>` head, so the
block-scope declarations of its body are not under it (`int i(n);` there is
a local, not an extraneous variable specialization; WP-803,
`temp.expl.spec/special-member-spec-body-locals-run.cpp`).

**Member classes and member enumerations of an implicit instantiation**
([temp.expl.spec]/1, /5, /7; WP-206), such as `template<> struct
A<int>::N {...}`, `template<> enum class A<int>::E : int {...}` and the
declaration-only `template<> struct A<int>::N;`:
1. The parser routes `template<>` followed by a class key or `enum` to
   the specialization declaration path. `act_on_tag` finds the member that
   the class instantiation declared and, inside the member-spec header,
   calls `check_member_tag_spec` (`templ/spec.rs`, clang
   `CheckMemberSpecialization` + `CheckSpecializationInstantiationRedecl`).
2. A member that is already instantiated (an implicit instantiation that
   has a definition, or any explicit instantiation) is "explicit
   specialization of 'N' after instantiation". The note is "implicit
   instantiation first required here" (or "explicit instantiation ...")
   at the member's point of instantiation, or else at the point of
   instantiation of the nearest enclosing specialization
   (`enclosing_poi`). After this error the declaration is dropped. A
   declaration-only specialization returns the existing member.
3. Otherwise the member's `MemberSpecInfo.kind` becomes
   `ExplicitSpecialization` and the new declaration redeclares it. A
   redefinition is clang's "redefinition of 'N'". `instantiate_member_class`
   never instantiates an explicitly specialized member, and its uses see
   the specialization's definition. An unknown name in the specialization
   is "no member named 'x' in 'A<int>::N'" (`find_instantiated_decl`).
4. **Lazy scoped member enumerations** ([temp.inst]/3). The class
   instantiation only declares a scoped member enumeration
   (`Substituter::enum_decl`). Its enumerators are instantiated
   (`instantiate_member_enum` → `enum_definition`) when the enumeration
   must be complete:
   - as a nested-name-specifier (`require_complete_nns` /
     `undefined_member_enum`);
   - in `using enum`;
   - when the pattern names one of its enumerators (`find_instantiated_decl`
     → `require_member_enum`).
   This is what makes a later specialization valid. Unscoped member
   enumerations are instantiated together with the class, so specializing
   one is always "after instantiation", as in clang.
5. `template enum ...`: a qualified name is "enumerations cannot be
   explicitly instantiated". An unqualified name, like `template<> enum
   E ...`, gives "enumeration cannot be a template" at the token after
   the name or base, plus "declaration does not declare anything" at
   `enum` (clang's parser).

**Out-of-line members of such a partial specialization** (deviation D7,
§1.5): `template<> template<class U, char d> int O<int,
char>::A<U*, d>::f() {}` after the partial specialization above is a
member definition of that partial specialization. The qualifier
`O<int, char>::A<U*, d>` is matched against the partial specializations
of the redeclared member template (parameter names may differ; the
deduced `U` and `d` reach the body), not merely accepted: a qualifier that
names no partial specialization (`A<T&, c>`), or a member the partial
specialization does not declare, is "out-of-line definition of 'f' does
not match any declaration". clang 19 and 22 reject the valid form, so the
test is `skip(clang)`: `temp.expl.spec/member-partial-spec-out-of-line-member-run.cpp`.

Not done: when the inner head does not match the member template's head
(for example, different parameter count), clang drops the declaration.
kiln still links it as a redeclaration, so later uses get cascading
errors.

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
   - the class-virt-specifier **`final`** of the pattern (the primary
     template's or the partial specialization's definition), through
     `act_on_class_final` before the bases (WP-446), so `__is_final` and
     the final-base check see it on the specialization. Test:
     `tests/conformance/temp.inst/final-class-template.cpp`.
   - A member function's declaration, and a function template
     specialization's (at `function_spec_signature`), gets the pattern's
     attributes (`class_attrs`; clang `InitFunctionInstantiation` →
     `InstantiateAttrs`; WP-182): `const`/`pure`, `[[noreturn]]`,
     `[[nodiscard]]`, `[[deprecated]]`, ... — so codegen, `-Wunused-result`
     and `-Wdeprecated-declarations` see them on the specialization. A
     dependent `alignas` operand is substituted; other arguments are
     shared with the pattern (no attribute kind that kiln evaluates takes
     a dependent operand on a function yet). A befriended function gets them
     too (a hidden friend found by ADL; WP-491); a declaration that
     already has attributes is skipped. `[[deprecated]]` names a function
     template specialization with its arguments (`'f<int>' is
     deprecated`, clang `getNameForDiagnostic`). A function template
     specialization also takes the `nodiscard`/`pure`/`const` of the
     template's other declarations (`function_spec_attrs`;
     spec/landed/sema/WP-491.md).
   - A member typedef or alias-declaration gets the pattern's `aligned`
     attributes, a dependent operand substituted (`var_align_attrs` in
     `typedef_decl_in`; clang `InstantiateAttrs`; WP-335): libc++'s
     `atomic_ref` names its object through `typedef _Tp _Aligned_Tp
     __attribute__((aligned(required_alignment)))`.
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
| anonymous struct or union, unnamed member class | definition, with the unnamed member object and the injected `IndirectField`s (`subst/anon.rs`) | — |
| unnamed bit-field | declaration, width substituted (a non-dependent width's `ConstantExpr` is not wrapped again) | — |

A member function whose definition is instantiated from an out-of-line
pattern definition (a user-written body, or `= default`) is located at that
definition from then on (clang `InstantiateFunctionDefinition` copies the
pattern definition's location and declarator end;
`AstBuilder::relocate_to_pattern_definition`, WP-428): diagnostics in the
instantiated body that point at the function (a reference member a
constructor does not initialize), its "in defaulted ... first required here"
note, and later candidate and "declared here" notes name the out-of-line
definition. A member whose definition was not instantiated stays at its
in-class declaration. Test:
`tests/conformance/temp.inst/out-of-line-definition-location.cpp`.

6. Complete the class:
   - implicit special-member declarations (S3C);
   - the layout, which stays lazy until abi asks for it.
7. Record `point_of_instantiation = poi` and set `kind =
   ImplicitInstantiation`.
8. `Ast::end_instantiation()`, record the report entry (§11.3), and pop
   the frame.

A member class completed on its own (`instantiate_member_class`) is
substituted with the arguments of its nearest enclosing specialization.
Every enclosing pattern class maps to its instantiation, up through
that specialization and the classes around it
(`inst_context_pairs`). So `view<V>::outer<C>::value_type` naming
`inner<C>` (a sibling member template of `view`) finds
`view<int>::inner` (WP-276).

Any error during instantiation marks the specialization `invalid` (P7).
Later uses of it are silent. As in clang's `InstantiateClass`, the class
body's own failures do so too (`class_body_with`): a failed base
specifier, an invalid data member (field or static data member) of the
pattern, which is skipped, or a data member whose instantiation fails.
A `static_assert` member that fails (its condition is false, not a
constant expression, or does not convert; `static_assert_checked`) also
stops the body: the members after it are not instantiated
(clang's `InstantiateClass` bails out on `isFailed`), so libc++'s
`optional<char[20]>` reports only its assertion. Other invalid members
(a member function whose type fails) do not stop it
(tests/conformance/temp.inst/failed-static-assert-stops-instantiation.cpp).
The specialization is then still completed with its implicit special
members (`ensure_implicit` exempts invalid specializations), its default
member initializers are not instantiated, and the use that required the
definition fails without a further "incomplete type" error
(`undefined_diagnosed`). `instantiation_fails` is that quiet failure
for the uses clang checks with `RequireCompleteType`: a variable
definition (default-initialized or with an initializer) or a function
parameter of the specialization's type is invalid, so its later uses
raise nothing and instantiate nothing (`a.g()` after `A<int> a;`), and a
member access that instantiated the class builds no expression, so the
member function it names is not instantiated. Later uses find the class
complete: a static member, a member pointer, or a later member call
instantiate their members as usual
(`tests/conformance/temp.inst/invalid-specialization-members.cpp`).

As clang's `TemplateDeclInstantiator`, a field, a member typedef or
alias, or an enumeration whose type (underlying type) fails to
substitute is still declared, `INVALID` (a field or typedef of the error
type, an enumeration of underlying type `int` with its enumerators), so
lookup finds it and its uses are silent (`member_ref_without_object`
builds nothing for an invalid declaration); a static data member or a
member function whose type fails is not declared, and naming it is "no
member named". Any member instantiated invalid makes the specialization
invalid, not only a data member. The other uses that clang checks with
`RequireCompleteType` fail the same quiet way when the instantiation
fails at them (`instantiation_fails`): a field of that type (also a
member class, whose instantiation now reports failure like a
specialization's) is invalid, and a base-specifier naming it is dropped
(class/members.rs), so the derived class does not find its members; a
using-declaration whose qualifier names an invalid class is not
diagnosed as "not a base class"; an explicit type conversion `T(a, b)`,
`T()` or `T{a, b}` (forms.rs, clang `BuildCXXTypeConstructExpr`) whose
instantiation of `T` happens there and fails builds nothing inside a
template instantiation (clang's `ExprError`); outside one the invalid
class is initialized quietly and its members are still looked up, as
clang's `RecoveryExpr` of type `T` is (`Y<int>().m()` is "no member
named 'm'") (`temp.inst/failed-static-assert-functional-cast.cpp`,
WP-446). A decl-specifier whose qualifier fails
so (`X<A>::type v;`) spans the scanned name and the declaration goes on
with an invalid type; a base-specifier `X<A>::B` is clang's "expected
class name" at `B`
(`temp.inst/invalid-specialization-invalid-members.cpp`,
`temp.inst/invalid-specialization-base.cpp`).

**Points of instantiation.**
- `SpecInfo.point_of_instantiation` is the location of the use that first
  required the definition, as clang records it. It is used only in
  "requested here" notes and in the explicit-specialization-after-
  instantiation check.
- Function and variable definitions that are not needed immediately go on
  the `pending` worklist with their POI. `end_of_tu_instantiations()`
  drains the worklist with a loop; an instantiation may push more entries
  onto it.
- Each definition the drain instantiates is recorded in
  `TemplateTables::end_of_tu_defs` for codegen (spec/codegen.md §2a),
  in clang's order ([temp.point]/7: the end of the TU is a point of
  instantiation). A function definition is recorded before the
  instantiations it requires (pre-order, clang
  `InstantiateFunctionDefinition`'s `HandleTopLevelDecl`). A variable
  definition is recorded after them (post-order, clang
  `InstantiateVariableDefinition`'s `PassToConsumerRAII`): a static data
  member's out-of-line redeclaration and a variable template
  specialization alike, including one whose initializer was instantiated
  in place on the specialization itself (WP-440/WP-466,
  `pending.rs` `instantiate_pending` returns it to the drain, which
  records it once that definition's nested queue has drained;
  `tests/codegen/var_template_init_order.cpp`).
- A variable definition instantiated at a reference while the TU is
  parsed (one usable in constant expressions, below) is handed to codegen
  there, as clang's `PassToConsumerRAII` does (WP-490,
  `note_var_referenced` → `record_ref_var_inst`):
  `TemplateTables::ref_var_insts` records it with the namespace-scope
  declaration containing the reference ([temp.point]/1), so codegen emits
  it right before that declaration. The anchor is the enclosing
  namespace's member containing the current context (or the current
  function), else the namespace-scope variable whose initializer is being
  analysed (`depr_init`, `initializing`); a reference in a declaration
  that is not a member (a `static_assert`) anchors after the namespace's
  last member. References inside another instantiation, or during the
  end-of-TU drain, are not recorded: codegen emits those at first use.
  `tests/codegen/var_template_point_of_instantiation.cpp`.

### 4.7 Injected-class-name

- Inside the pattern, `A` names an `InjectedClassName` type. Its
  canonical type is the dependent `A<T…>` (types.md T6).
- `A` followed by `<`, or used as a template template argument, is the
  template-name ([temp.local]/1–2). The parser forms such an argument as
  a type; `template_id` converts it to the template when the
  corresponding parameter of a class, alias or variable template or of
  a concept is a template template parameter (`injected_template_args`, clang's
  `convertTypeTemplateArgumentToTemplate`), so substitution never sees
  the injected type there (libc++ `__split_buffer:160`
  `__split_buffer_pointer_layout<__split_buffer<…>, …>`, WP-398). Test:
  `tests/conformance/temp.local/injected-class-name-template-template-arg.cpp`.
  An argument that does not name a template for a template template
  parameter — `H<Y<T>>` (a specialization), `H<T>`, `H<N>`,
  `H<T::X>` without `template` — is rejected when the enclosing template
  is defined, at the argument, even though the arguments are dependent
  (`check_dependent_template_args`, run by `template_id` before it
  forms the dependent specialization; clang's
  `CheckTemplateArgumentList` runs on dependent arguments too, WP-404,
  §10.5). Alignment stops at the first pack expansion. Test:
  `tests/conformance/temp.arg.template/dependent-non-template-argument.cpp`.
- A template argument `ns::A` or `C::A` whose qualifier is a namespace
  or the class `A` is a member of finds the template itself, never an
  injected-class-name, so inside `A`'s own definition it is a template
  template argument too (the parser's `names_member_template`; libstdc++
  `_UnClos<__negate, std::_Expr, _Clos>` in `_Expr`, WP-332). Test:
  `tests/conformance/temp.local/libstdcxx-qualified-template-name-in-own-class.cpp`.
  The same holds for a deduced class type placeholder: `::n::A{a, b}`
  or `n::A(a)` inside `A`'s own definition, in an expression or a
  decl-specifier, deduces `A`'s arguments from its initializer as it does
  outside (the parser's `names_ctad_template`; mp-units' delegating
  constructor `quantity(::mp_units::quantity{v, R2{}})`, WP-792). Test:
  `tests/conformance/temp.local/qualified-name-in-own-definition-ctad.cpp`.
- The injected-class-name of a base class template *specialization*,
  found by lookup in a derived class (`V` inside `struct B : V<long>`,
  `B::V`, `Doc::V` with `Doc` a typedef of a derived specialization) and
  not followed by `<`, is a type-name naming that specialization
  ([temp.local]/1, [class.qual]/2): lookup finds the specialization's
  class, `classify_name` gives the class template (right for `V<int>`),
  and the parser's name scan and qualified type-name turn it back into
  the specialization's type (`Sema::injected_specialization_name`;
  rapidjson's `Document::GenericValue`, WP-704). Not for the
  injected-class-name of the class whose scope we are in (the bullets
  above). Test:
  `tests/conformance/temp.local/base-specialization-injected-name.cpp`.
- Substituting an `InjectedClassName` gives
  `Record(get_class_spec(tmpl, level args))`. It always hits the map,
  because the specialization exists.
- Inside a partial specialization, the injected-class-name is the
  partial's own pattern args, as written: the argument pack the
  conversion formed for the primary's trailing parameter pack is spread
  into its elements (`vd<int, T...>`, not `vd<int, <T...>>`), so a
  substitution (a member alias template naming `vd`) converts them
  against the primary's parameters again (`partial_injected_type`;
  WP-787,
  `tests/conformance/temp.local/partial-pack-injected-name-in-alias-template.cpp`).
- As a nested-name-specifier (`A::*`, `&A::m`, `A::m`) it is the
  `InjectedClassName` type too, so member pointers formed from it are
  dependent and substituted (spec/sema.md §5.21, WP-608).

### 4.8 Friends

- A friend function **defined** in a class template (a hidden friend) is
  instantiated as a declaration into the innermost enclosing namespace,
  visible only to ADL. Its body is instantiated on odr-use. It is
  instantiated through its `FriendDecl` only (WP-082), not also as an
  entry of the class's member list. A defaulted friend comparison is
  instantiated as defaulted and checked at the specialization's `}`
  (sema-class.md §4.12).
- If its signature does not depend on the template's parameters, the
  second specialization to instantiate it reports "redefinition of 'f'"
  (clang). The note shows the first instantiation.
- A friend function template defined in a class template has its
  specializations instantiated from that definition even when the class
  specialization was instantiated before the body was parsed (a body of
  the class naming `M<int>`, [class.mem.general]/7): the parser records
  the saved body (`Sema::act_on_delayed_body`), and the instantiated
  friend is linked to the pattern as if the body were there (clang's
  `willHaveBody`), rather than left a declaration whose use is "function
  'pow' with deduced return type cannot be used before it is defined"
  (WP-792; `temp.friend/friend-template-body-names-own-specialization.cpp`).
- A friend function declared (unqualified) in a templated class
  redeclares nothing when the template is defined: it is not linked
  to an earlier declaration of the same name, nor checked against an
  earlier definition (clang). Its instantiation is linked to the
  namespace-scope declarations; if one of them, or an earlier
  instantiation, already has a body, the instantiated definition
  reports "redefinition of 'g'" with the instantiation note and
  "previous definition is here".
- A friend template, or a friend specialization (`friend void f<>(T)`),
  is substituted and then matched against the namespace-scope templates
  when it is instantiated. The friend template keeps its pattern's
  constraints (checked through `member_constraints`), whose own
  parameters are a level deeper than the namespace template's; while it
  is matched, each type-constraint and requires-clause is compared as its
  substitution with the specialization's arguments, the friend's own
  parameters at their retained depth (`Sema::friend_cmp_constraints`;
  clang's `AreConstraintExpressionsEqual`). A constraint whose
  substitution is not dependent names only the enclosing class's
  parameters ([temp.friend]/9): it is compared as written, so the friend
  stays a template of its own (WP-446; libc++ 22 `stack`/`queue`
  `operator<=>`; `temp.friend/constrained-friend-template-*.cpp`). A
  constraint whose substitution used an enclosing level's argument (or
  the injected-class-name's instantiated class), even one that also
  names the friend's own parameters, makes the friend a template of its
  own: no earlier declaration is looked up, so `template<class Self,
  class U = T> requires is_same_v<remove_cvref_t<Self>, W> friend U
  get(Self&&)` defined in `W<int>` and `W<long>` declares two templates,
  without a "redefines default argument" or "redefinition" error
  (`Sema::friend_template_apart`, `Substituter::outer_used`; clang's
  `FriendConstraintsDependOnEnclosingTemplate`; WP-726,
  `temp.friend/p9-constraint-on-enclosing-distinct.cpp`). The constraint
  may be a template parameter's type-constraint
  (`template<std::convertible_to<T> U>`) or name a level further out
  (a member class template of a class template), or name a member class
  of the enclosing template, which substitutes to another class (the
  injected-class-name of `outer<C>::type`, defined in the class or out
  of line; WP-789,
  `temp.friend/constrained-friend-of-member-class-of-template-run.cpp`).
  A non-template friend
  with a requires-clause is apart the same way (the paragraph's "such a
  constrained friend function"; `friend_decl` sets the flag from the
  pattern's trailing requires-clause), so `friend int f(Base) requires
  (sizeof(T) > 0) {}` in `A<char>` and `A<int>` is two functions, not a
  redefinition (WP-779,
  `temp.friend/p9-constrained-friend-distinct-general.cpp`). Gap: the
  paragraph's two "must be a definition" errors (clang
  `err_non_temp_friend_decl_with_requires_clause_must_be_def`,
  `err_friend_decl_with_enclosing_temp_constraint_must_be_def`) are not
  diagnosed.
- A friend function or function template declared with a qualified name
  in a templated class (`template<class U> friend R N::f(X<U>);`,
  `friend R N::g(T);`) whose template head or type is dependent is not
  matched when the class template is defined: its own parameters are a
  level deeper than `N::f`'s and the match may depend on the class's
  arguments ([temp.friend]/1, [temp.over.link]/6). It is kept as a hidden
  pattern (like an unqualified one) and its qualifier recorded
  (`Sema::friend_qualifiers`); each instantiation of the class declares it
  again with that qualifier, so the substituted friend (now at `N::f`'s
  depth, parameters compared by position) redeclares the namespace member
  and befriends it, wherever the class itself lives. A non-template
  friend that matches nothing then is "friend declaration of 'g' does not
  match any declaration in namespace 'N'" with the instantiation note
  (clang adds a candidate note kiln does not emit); a friend template
  that matches no template is accepted silently, as clang does
  (`Sema::instantiating_friend_template`). At definition, a qualified
  name that finds nothing at all is diagnosed at once (clang), as is a
  mismatch in a non-template class or of a non-dependent friend
  (WP-273; `temp.friend/qualified-friend-template-*.cpp`,
  `qualified-friend-mismatch-at-instantiation.cpp`).
- A friend class (`friend class B<T>;`) is substituted and grants access
  (S3C). A dependent friend specialization (`friend struct X<T>;`,
  `friend class Y<T*>;`, `friend class ns::Z<T...>;`) is kept in the
  pattern's `FriendDecl` as a dependent `FriendTarget::Type`; each
  instantiation of the class substitutes it and befriends the resulting
  specialization (WP-136). A non-dependent one (`friend struct X<int>;`)
  names the specialization directly, in the pattern as outside a
  template. In the pattern (or a partial specialization) of `X`, a
  member access befriended through a specialization of `X` is accepted:
  whether the instantiation is the befriended one is decided then
  (clang's dependent access result). Such a check is recorded against the
  innermost pattern function or class (`Sema::dependent_access`) and
  re-run in each instantiation of it (a class at the end of its
  instantiation, a member function after its body; clang
  `PerformDependentDiagnostics`, WP-151); an error the instantiation
  already reported is not repeated. The innermost entity skips a
  non-generic lambda's call operator and closure type (instantiated with
  their enclosing function: a lambda in `X<char>::l` replays with `l`); a
  function template specialization (`X<char>::m<int>`, a generic lambda's
  `operator()<int>`) replays its pattern's checks after its body, and a
  generic lambda rebuilt by substitution inherits its pattern's checks
  (`Sema::copy_dependent_access`), so the error carries the
  `in instantiation of function template specialization` note (WP-174).
  A replayed check's [class.protected] object class is substituted as
  clang's `BaseObjectType` is: when it is the pattern (or a member class
  of it), it becomes the instantiated class of the replay's context, so
  `using B::m;` and an implicit `B::m` naming a protected member of a
  non-dependent base reached through a dependent base are accessible in
  `R<int>` (the implicit access records `*this`'s class as its object;
  `class.protected/dependent-base-current-object.cpp`, WP-680).
  Type names are checked as other names (spec/sema-class.md §2.4).
- `template<class U> friend struct Y<U*>;` is "partial specialization
  cannot be declared as a friend" ([temp.friend]/9).

#### 4.8.1 Which declaration's head is read (the shared rule)

**Template parameter lists are always read from the head/defining
declaration, never from `ast.first(t)`.** A class template's first
declaration may be a friend declaration inside a class template
(`template <class C, class Tr> friend class sv;` in `wrap<I>`, as libc++
19 declares `basic_string_view`, `basic_string` and `vector` first in
`__wrap_iter`): it redeclares the namespace scope template
([temp.friend]/1), but its parameters sit one level (or more) deeper, may
have other names, and carry no default arguments ([temp.param]/12).
`ast.first(t)` stays the template's identity (the key of
`specializations`, `partials`, caches); everything that reads the
parameters, their defaults or their depth goes through one of:

- `head_template(t)`: the definition's template, else the earliest
  declaration that is not a friend, else the first. Parameters, defaults
  and depth for argument checking (`check_template_args`, §4.2), default
  completion of dependent template-ids (§6.2), default suppression in
  printed names, outer levels, and the partial specialization check that
  the arguments are not the primary's own parameters
  ([temp.spec.partial.general]/9.3, "class template partial
  specialization does not specialize any template argument";
  `check_partial_args`). A non-type argument names the primary's
  parameter only when the partial specialization's parameter also has
  the primary parameter's canonical type ([temp.over.link]/6, clang's
  profile of a non-type parameter includes its type): `template<auto K>
  class C; template<class T, T *K> class C<K>` specializes `K` (WP-789,
  tests/conformance/temp.spec.partial.general/auto-primary-same-type-partial-is-primary.cpp).
- `visible_template(t)`: the latest declaration that is not a friend
  (the one name lookup finds; a friend is invisible to it), else the
  first. The constraints checked for a class template-id
  ([temp.names]/8, §8.3) and the "template is declared here" note of an
  arity error: clang names that declaration's head (`[with C = char]`,
  its requires-clause), never a friend's. Satisfaction is the same for
  every declaration (equivalent heads, [temp.over.link]/6).

CTAD forms its guides from the definition's template (§6.6). A friend
*function* template in a templated class redeclares nothing until the
class is instantiated, and then with its head substituted to the
namespace template's depth (§4.8), so function deduction, address
deduction and partial ordering (`deduce_spec`, `more_specialized_in`)
read a declaration at the right depth through `ast.first` and need no
redirect. Audited (WP-299): `deduce_call.rs` `deduce_spec`/
`deduce_conversion`, `order.rs` (function and partial ordering),
`ctad.rs` (implicit, aggregate, user and copy guides), `ttarg.rs`
(template template argument matching), `spec.rs`/`actions.rs`
(specialization, instantiation, partial specialization matching),
`funcs.rs`; only the partial specialization check, the class constraint
check and the arity note read a friend's head, and are redirected. Tests:
`tests/conformance/temp.friend/friend-first-class-template-uses.cpp`,
`tests/conformance/temp.friend/friend-first-head-diagnostics.cpp`,
`tests/conformance/temp.deduct.call/friend-first-function-template.cpp`,
`tests/conformance/over.match.class.deduct/friend-first-class-template.cpp`,
and the §6.2 tests.

### 4.9 Default member initializers

- The NSDMI of a member of a specialization is instantiated the first
  time a constructor that uses it is defined, or an aggregate
  initialization reads it. It is cached in the field's cold slot.
- The fields of an anonymous struct or union member are deferred the same
  way: `sizeof(V<P>)` does not instantiate `P a = nullptr;` inside an
  anonymous struct (WP-446; libc++ 22's `_LIBCPP_COMPRESSED_PAIR`,
  `tests/conformance/temp.inst/nsdmi-anonymous-member-deferred.cpp`).
  Only a local class's initializers are instantiated with the class.
- A recursive use while the enclosing class is being instantiated is
  clang's error: "default member initializer for 'x' needed within
  definition of enclosing class 'A<int>' outside of member functions".
- The frame is `DefaultMemberInitializerInstantiation`.
- A braced NSDMI `T m{x}` whose pattern is already a construction (`x`
  only value-dependent) is rebuilt from its braced list, as a variable's
  initializer is (§5.4, `Substituter::initializer`), so it stays
  direct-list-initialization and an explicit constructor is usable
  (WP-787, Boost.Hana `Tracked tracker{i}`,
  `tests/conformance/class.mem.general/default-member-initializer-direct-list-dependent.cpp`).
- An NSDMI whose substitution fails makes its field invalid (clang
  `InstantiateInClassInitializer`), so a constructor or aggregate
  initialization that uses it reports nothing more (WP-220;
  `default_member_init` returns `None` for an invalid field). The error's
  notes are clang's (WP-238, `tests/conformance/class.mem/nsdmi-subst-failure.cpp`,
  `nsdmi-lazy-context-notes.cpp`): the initializer's frame at the
  constructor (an out-of-line instantiated constructor at its pattern's
  definition, as clang `InstantiateFunctionDefinition` relocates it), then
  the constructor's instantiation or the exception-specification
  evaluation that needed it. An implicit special member's exception
  specification is resolved before it is defined, so an implicit default
  constructor's initializers are first instantiated by its exception
  specification. Open: aggregate initialization keeps filling the
  remaining fields after a failed initializer (clang stops at the first;
  `init.rs` `field_filler`).

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
- Which alias failures are SFINAE (WP-274, clang 19): everything found
  while substituting the alias's arguments into its aliased type under a
  `Some(true)` frame — a missing member type (`enable_if<false>::type`,
  `T::type`), an invalid type, a failed nested alias. A failure inside a
  class *definition* the alias instantiates (a member of `enable_if<..>`
  whose body is ill-formed) is a hard error under that class's
  `Some(false)` frame. Outside deduction (a class member's type, a
  non-template) the alias's failure is a hard error with the "in
  instantiation of template type alias" note. The enable_if diagnostics
  are §5.7.1.
- A pack expansion as the argument for a non-pack parameter is
  diagnosed when the alias is dependent: "pack expansion used as argument
  for non-pack parameter of alias template" (clang, CWG1430).
- **Locations and access (WP-194).** clang substitutes an alias's
  aliased type with trivial `TypeLoc`s, so every error found in it is
  reported at the alias's `using` keyword (`templ.alias_using_loc`), not
  at the names as written (§5.8 is off: `Substituter::written_locs =
  false`). The "in instantiation of template type alias 'X' requested
  here" frame is still pushed at the use. Access checks
  ([class.access]/4) use clang's context: the use site for an alias at
  namespace scope, the alias's class for a member alias (clang's
  `CheckTemplateIdType` enters the alias's context only when it is not a
  file context). The override is `Sema::access_ctx`, read by
  `access_start` and cleared by `in_inst_context`.
- clang re-substitutes the alias at every use, so an access error repeats
  per use. kiln does not cache a result whose substitution ran an access
  check (`Sema::access_checks` changed), and a check made directly in an
  alias frame is emitted with `allow_duplicate` (spec/diag.md §7.4).
  The SFINAE outcome of such a use is therefore never taken from the
  cache.

### 4.11 Variable templates and static data members (and the Oven)

**Variable templates.** `v<args>` calls `get_var_spec`, which is §4.2
with a `VarTemplateSpecialization` decl.
- The type is substituted immediately: the expression needs it.
- Partial specializations are selected as in §4.4.
- A template declared with a placeholder anywhere in its type (`auto`,
  `auto&`, `const auto*`; `contains_undeduced_auto`, not only a
  top-level `auto`) instantiates its initializer at the specialization
  to deduce the type, so a use of `v<T>` inside another constexpr
  initializer is not "depends on itself" (WP-726;
  `tests/conformance/dcl.spec.auto/variable-template-reference-placeholder.cpp`).
- A class member access `x.v<args>` / `p->template v<args>` naming a
  static data member template ([temp.names]/3, [expr.ref]/6.1) is held as
  a one-member set until its arguments follow; non-dependent ones name
  the specialization, accessed as a static data member (clang
  `BuildMemberReferenceExpr` → `CheckVarTemplateId`), dependent ones form
  the access again at instantiation, and none at all is "use of variable
  template 'v' requires template arguments" (WP-789,
  `tests/conformance/temp.names/member-access-variable-template-run.cpp`).
- The `inline constexpr bool xxx_v` family (142 declarations in the M3
  headers, §13.3) is the hottest path. One probe gives the decl and one
  `consts()` lookup gives its value.

**Initializers.** Static data members and variable specializations follow
the same rule, which is clang's `DoMarkVarDeclReferenced`.
- A variable that is *usable in constant expressions* (`constexpr`, or
  const integral/enum with an initializer, or a reference) has its
  initializer instantiated **immediately** when it is referenced. Its
  value is then evaluated and cached in `consts()`.
- Such a variable referenced before its template's (or its static data
  member's) definition is instantiated from the definition at the next
  reference after it (WP-490; clang `InstantiateVariableDefinition` finds
  the definition then), so `template<class T> extern const int e; int
  g() { return e<int>; } template<class T> const int e = 12;
  static_assert(e<int> == 12);` holds.
- **Deviation** (WP-490, docs/status/deviations.md): one odr-used only
  before the definition is also queued, quietly (`InstState.quiet_pending`),
  and defined at the end of the TU, which is a point of instantiation too
  ([temp.point]/7, [temp.inst]/7). clang never queues it (its
  `DoMarkVarDeclReferenced` FIXME) and GCC does not define it either: both
  leave the symbol undefined, a link error. The quiet entry never warns
  (`-Wundefined-var-template` stays clang's, below).
  `tests/conformance/temp.point/var-template-end-of-tu-instantiation-run.cpp`,
  `var-template-defined-after-reference-run.cpp`.
- A static data member's instantiated out-of-line definition keeps the
  pattern definition's `constinit` ([dcl.constinit]/1): it is the
  initializing declaration with `constinit`, so `template<class I>
  constinit T C<I>::m{};` gives no "'constinit' specifier missing"
  warning.
- Other variables are queued in `pending` when they are odr-used.
  That includes an inline static data member whose in-class initializer
  was deferred (§4.4; `deferred_inits`): the drain instantiates its
  initializer in queue order, interleaved with the out-of-line static
  data members and function templates queued around it, so the
  diagnostics and the emitted `__cxx_global_var_init` functions follow
  clang's order (WP-186;
  `tests/conformance/temp.inst/static-member-instantiation-order.cpp`,
  `tests/codegen/static_member_inst_order.cpp`). The member was declared
  without its initializer, so it takes the pattern's init style when the
  initializer is instantiated: `static constexpr E e{X::v};` stays
  direct-list-initialization ([dcl.init.list]/3.8, an explicit
  constructor is viable) and `= {X::v}` copy-list (WP-690;
  `tests/conformance/class.static.data/template-direct-list-init.cpp`,
  `template-copy-list-init-errors.cpp`). A member whose type comes from
  its initializer is never deferred: a placeholder (`auto`, `auto*`,
  `const auto&`), a deduced class type (`std::array const tbl =
  f<T>();`, clang's `isUndeducedType`; WP-792,
  `tests/conformance/temp.inst/static-member-deduced-class-type.cpp`) or
  an array of unknown bound.

This is why the chain `R<N>::v = R<N-1>::v` recurses one frame per level
in clang (the d1 test in §10.2). Kiln matches its notes exactly.

**Undefined variables at the end of the TU** (`-Wundefined-var-template`,
WP-220; clang `InstantiateVariableDefinition`). When the pending queue is
drained (`instantiate_pending`) and an implicitly instantiated variable
specialization or static data member still has no definition to
instantiate from, kiln warns "instantiation of variable 'v<int>' required
here, but no definition is available" at its point of instantiation,
with the notes "forward declaration of template entity is here" (the
pattern's latest declaration) and "add an explicit instantiation
declaration to suppress this warning if 'v<int>' is explicitly
instantiated in another translation unit" (the POI again). The rules
follow clang:
- Only odr-uses queue the variable; a variable usable in constant
  expressions is never queued (it is instantiated at the reference), so
  it is never warned about.
- The POI is the first reference, even an unevaluated one
  (`InstState.undefined_var_uses`).
- A variable template specialization is queued again on every odr-use,
  so each use warns again (same POI; `allow_duplicate`). A static data
  member is queued once.
- No warning for explicit specializations, explicit instantiation
  declarations (`extern template int v<long>;`), members of a class
  named in `extern template struct S<char>;`
  (`in_explicit_inst_declaration`), patterns in a system header, or once
  any error has been reported (clang checks `hasErrorOccurred()` at the
  end of the TU).
- An explicit instantiation definition of the class (`template struct
  U<int>;`) leaves an undefined static member implicit, so it warns.

`tests/conformance/temp.inst/undefined-var-template.cpp` pins all of
this. A variable template defined later in the TU is instantiated
silently: its queued implicit specialization finds the definition seen
since (`instantiate_pending` -> `instantiate_late_var_spec`, clang
`InstantiateVariableDefinition`) and is recorded in `end_of_tu_defs`, so
codegen emits it as its own linkonce_odr definition (WP-238,
`tests/codegen/var_template_defined_after_use.cpp`). One usable in
constant expressions (`const int`) is not queued and stays an external
declaration, as in clang.

**Member variable templates defined out of line** (`template<class T>
T A::v = T(1);`, WP-206). The implicit specialization `A::v<int>` is a
single declaration in `A`. Its initializer is instantiated from the
out-of-line definition, so the in-class static initializer check
([class.static.data]/3) applies only when the pattern was written in the
class (`init_written_in`; clang's `BuildVariableInstantiation` keeps the
pattern's lexical context). Codegen emits it as a definition
(`instantiated_var_def`).

**Explicit instantiation of a variable template specialization**
(`template int vv<long>;`, `extern template ...`; `act_on_explicit_var_inst`,
clang `ActOnExplicitInstantiation`). The steps run in clang's order:
1. The redeclaration check (`check_explicit_var_inst_redecl`, the §4.5
   table): "duplicate explicit instantiation of 'vv<long>'", "explicit
   instantiation declaration (with 'extern') follows explicit
   instantiation definition (without 'extern')", the warning "explicit
   instantiation of 'vv<long>' that occurs after an explicit
   specialization has no effect". An `extern template` after an explicit
   specialization or another explicit instantiation declaration does
   nothing and says nothing.
2. The new kind and the first point of instantiation are recorded only
   when the instantiation has an effect.
3. The declared type is compared with the specialization's type
   (`check_explicit_var_inst_type`) even when step 1 found no effect:
   "type {0} of explicit instantiation of 'vv<long>' does not match
   expected type {1}" at the start of the type specifier, with the note
   "variable template 'vv' declared here" at the template's latest
   declaration. A mismatch also stops the definition from being
   instantiated. Static data members and explicit specializations are
   not type-checked.

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
   - a member that was marked while its pattern was undefined
     (`InstState.queued`) and whose pattern is **still** undefined at
     the evaluation: `templates/diag/temp.inst-constexpr-still-undefined.cpp`
     pins the note.
4. As landed by WP-484 ([temp.inst]/8, GCC 16.1 as the oracle): a
   queued **constexpr** member whose pattern **is** defined by the time
   of the evaluation is instantiated on demand, as in step 2 (POI = the
   evaluation's location); its queue entry then finds it defined at the
   end of the TU. Nothing else changes: non-Oven instantiation keeps its
   timing and the queue is never reordered.
   `templates/diag/temp.inst-constexpr-defined-later.cpp` (`oracle:
   gcc`) pins it; clang 19/22 still report the function undefined, which
   is a clang limitation. libstdc++ 16.1 needs it: `operator""s` queues
   `basic_string::_M_construct<const char8_t*>` before `basic_string.tcc`
   defines it, and every `std::meta::exception` builds a `u8string`.

**Variable template specializations as declarations (WP-417).**
- `[[deprecated]]`: an explicit specialization carries its own
  attribute (before or after the declarator-id); an implicit one
  inherits the attribute of the partial specialization or primary
  template it is instantiated from; the primary's attribute does not
  reach an explicit specialization declared without one
  (`own_deprecation`; `dcl.attr.deprecated/var-template-spec.cpp`).
- [dcl.init.general]/8: an implicit specialization instantiated from a
  definition without an initializer is default-initialized, so a
  `const` one of a type without a user-provided default constructor is
  the error at the definition, noted at the use (primary, partial
  specialization, out-of-line static data member template). An
  explicit specialization of a static data member without an
  initializer is a declaration ([temp.expl.spec]/14), and an
  out-of-class member partial specialization without one is not
  instantiated as a definition (`is_spec_of_out_of_line_def`;
  `dcl.init.general/const-default-init-var-template.cpp`).
- [temp.expl.spec]/14 (WP-572): that declaration
  (`template<> int C<char>::n;`, `template<> int S::v<char>;`) is not a
  definition for any purpose: `Ast::is_member_spec_declaration` (shared
  by Sema's `is_var_definition`, the Oven's notes and codegen) says so,
  it is not default-initialized nor emitted (uses name an `external
  global`), a later one with an initializer (`= 1`, `{}`) is the
  definition rather than a redefinition, and one without an initializer
  after the definition only redeclares it (`act_on_static_member`).
  `template<> X C<int>::x();` declares a function ([dcl.ambig.res]/1):
  like any out-of-line member function declaration whose class lookup
  finds only a non-function, non-tag member (a variable, data member,
  type alias or enumerator), it is "redefinition of 'x' as different kind
  of symbol", noted at the member (clang `MergeFunctionDecl`;
  `non_function_member`), not "out-of-line declaration ... does not
  match" (`temp.expl.spec/static-member-spec-declaration-{ir,run,errors}.cpp`).
- [dcl.spec.auto.general]/11: a deduced-type variable template, partial
  specialization or explicit specialization named in its own
  initializer gets clang 22's wording ("variable template partial
  specialization 'ap<T *>' declared with deduced type 'auto' cannot
  appear in its own initializer", `sema_auto_var_*_in_own_initializer`,
  `check_var_template_own_init`); the use of a specialization of the
  primary is diagnosed once, in the pattern
  (`dcl.spec.auto/own-init-var-template{,-19}.cpp`).
- [dcl.spec.auto.general]/11, [temp.inst]/7, a specialization still
  being deduced, used from an initializer (WP-424). The specialization
  the instantiation makes for that use has no initializer yet: "declaration
  of variable 'w' with deduced type 'auto' requires an initializer"
  (`undeduced_var_spec_without_init`, from `var_init`), skipped when the
  pattern was already diagnosed. From clang 22 on (`clang_compat >= 22`)
  an implicit specialization's own use during its instantiation, and
  every later use of an invalid one whose type is still undeduced, is
  "the type of variable template specialization 'v<int>' declared with
  deduced type 'auto' depends on itself"
  (`sema_auto_var_template_spec_depends_on_itself`, from `get_var_spec`
  and `auto_expl_spec_in_own_init`; `var_spec_depends_on_itself`), at
  the use, silent when the pattern is invalid. Below clang 22 an implicit
  specialization's own use is a reference of type `auto`
  (`undeduced_var_spec_use_19`): an operator on it has "invalid operands
  to binary expression ('auto' and 'int')", and the specialization keeps
  the type `auto` (`deduce_auto` of an `auto`-typed initializer), so its
  uses fail to convert ("cannot initialize a variable of type 'int'
  with an lvalue of type 'auto'" at the declarator). clang 19 accepts a
  program whose uses convert nothing and crashes generating it; kiln
  reports clang 22's "depends on itself" at the point of instantiation
  at the end of the TU when no other error was reported
  (`TemplState::self_typed_var_specs`, `reject_self_typed_var_specs`),
  and a non-template variable deduced from such a reference is
  "variable 'y' with type 'auto' has incompatible initializer of type
  'auto'" (clang 19 accepts it). Tests
  `dcl.spec.auto/own-init-var-template-spec{,-19,-19-accepted}.cpp`.

**Requirement R-OVEN-1 (reentrancy)**, as landed:
- `Sema.oven` is an `Option<Box<Oven>>` (boxed since WP-645).
  `with_oven` takes it out for the duration of an evaluation and hands
  the closure a `Bridge` that holds `&mut Sema`.
- In `ensure_body`, the Oven passes its own `&mut Oven`. Sema lends it
  back by swapping it into `Sema.oven` (a fresh `Oven` with the same
  budget holds its place; the box is kept in `Sema.spare_oven` between
  lends) while it instantiates. Nested evaluations
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
- A reflection intrinsic lends the Oven the same way (`lend_oven`).
  `std::meta::substitute` instantiates a variable template
  specialization under a Transactional Oven checkpoint and a SFINAE
  frame; the specialization's initializer is evaluated there on the
  lent Oven as an ordinary manifestly constant-evaluated evaluation, not
  a trial, so its trait queries run (spec/reflection.md §4.3, WP-538).

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
| `Record`/`Enum`/`Typedef` of a decl in a dependent context | `find_instantiated_decl(d)`: a local goes through `locals`, and a local in `failed_locals` (its declaration statement failed: the declarator and the rest of its statement) is a silent substitution failure, already diagnosed ([temp.inst]/3; WP-316); a member goes through `member_map` of the enclosing specialization (of the instantiated parent, else of its defined redeclaration: a member class declared before its definition has two instantiated declarations and only the defined one records members; WP-582), and the member class is instantiated if completeness is needed |
| `Auto` | a deduced `Auto` substitutes its deduced type; an undeduced one stays and is deduced in the instantiation |
| `QualifiedArray{quals, base}` | cv applied to array sugar (`const A` with `using A = T[3]`): substitute `base`, then `qualify` pushes `quals` into the new element type ([dcl.array]/6; WP-301) |

`DependentBitInt{signed, width}` substitutes the width expression and
rebuilds the type through `Sema::build_bit_int_type`, which checks the
width then (spec/types.md §21; WP-404). A dependent `Splice` type
substitutes its operand and re-runs `Sema::act_on_splice_type`; the
dependent `CXXSpliceExpr` (an expression or member splice) and
`Nns::Splice` likewise re-run `act_on_splice_expr` /
`act_on_member_splice` and `act_on_nested_name_splice`, so the
designated entity is checked in the specialization (spec/reflection.md
§5; WP-474). Any other kind gives the NYI diagnostic "substitution into
this type".

### 5.4 Expressions

For each kind, the transform substitutes the children and then calls the
**same `Sema` action the parser calls**, e.g. `act_binary`, `act_call`
and `act_member`. Every check and implicit conversion is therefore
recomputed exactly as for non-template code (clang's
`TreeTransform::Rebuild*`).

- **Implicit nodes are stripped, then rebuilt by the action.** This
  covers `ImplicitCastExpr`, `MaterializeTemporaryExpr`,
  `CXXBindTemporaryExpr`, `ExprWithCleanups` and the `ConstantExpr`
  wrappers. `ParenExpr` is kept. An implicit `UserDefinedConversion`
  cast through a conversion function is stripped down to the call's
  object argument (`Substituter::udc_object`, also in
  `peel_initializer`; clang `CastExpr::getSubExprAsWritten`), so the
  enclosing action performs the conversion again from the substituted
  object. The call itself is never re-looked-up: a conversion function
  template specialization keeps the pattern's name `operator T` (§10.1),
  whose `T` is the member template's own parameter, and substituting it
  with the enclosing instantiation's arguments of the same depth and
  index named `operator Self` (`no member named 'operator E' in 'acc'`
  inside `bind<E>`; WP-563, [temp.deduct.conv]/1,
  tests/conformance/temp.deduct.conv/conversion-template-in-enclosing-template-*.cpp).
- **Sharing.** A non-dependent subtree is always shared (§3.3; there is
  no switch). The checks clang would redo by rebuilding it are replayed
  from what the definition recorded: designator warnings of a list
  (WP-196) and `-Wnull-dereference` (spec/sema.md §5.15, WP-235).
  A shared operand can sit inside a full-expression that is rebuilt
  (`B b(S{2});`, `use(t, S{1})` with `t` dependent). Its temporaries
  still belong to that full-expression. `note_shared_temporaries` sets
  `cleanup.needs` for each `MaterializeTemporaryExpr`. It sets `needs`
  and `side_effects` for each `CXXBindTemporaryExpr`, and does what
  `note_default_use` does for each `CXXDefaultArgExpr`. The walk skips
  nested `ExprWithCleanups`, lambdas and unevaluated operands. The rebuilt
  full-expression therefore gets its `ExprWithCleanups`, and the
  temporaries die at its end ([class.temporary]/4). Before WP-310 they
  died at scope exit, because the cleanup flag that clang's rebuild sets
  (`MaybeBindToTemporary`) was never raised;
  `tests/conformance/class.temporary/run-template-shared-operand-temporary-destroyed.cpp`.
- **Dependent forms** are resolved:
  - `DependentScopeDeclRefExpr` becomes qualified lookup;
  - `UnresolvedLookupExpr` becomes an overload set plus ADL (§3.5);
  - `CXXDependentScopeMemberExpr` becomes member lookup;
  - `CXXUnresolvedConstructExpr` becomes [expr.type.conv];
  - `ParenListExpr` becomes the initialization of its variable.
- **A resolved `MemberExpr` naming a member function template
  specialization** is rebuilt from the member name and the template
  arguments *as written* ([temp.arg.explicit]/4; clang
  `TreeTransform::TransformMemberExpr` passes only the written
  `TemplateArgs`), never from the arguments deduction chose: overload
  resolution runs again in the instantiation, so `s.str("")` with a
  `template <class A> void str(const basic_string<C, T, A>&)` overload
  deduces again and fails quietly instead of naming
  `basic_string<char, char_traits<char>, char[1]>`, which a deduced
  argument list replayed as explicit would instantiate (libc++
  `ostream.formatted.print/locale-specific_form.pass.cpp`, WP-358). An
  implicit member access (`f<X>()` inside a member, an implicit `this->`)
  keeps the written arguments on the `MemberExpr` it forms
  (`finish_overload_ref`, `set_member_template_args`). Test:
  `tests/conformance/temp.arg.explicit/deduced-member-template-args-not-rewritten.cpp`.
- **An NTTP reference** becomes `SubstNonTypeTemplateParmExpr {param,
  pack_index, replacement}`. The replacement is:
  - an `IntegerLiteral` (or the matching literal) built from the
    `ConstValueId`; for a `_BitInt(N)` (any N; WP-465) too, and for a
    wide `_BitInt` (N > 128) or a value outside `i128` (an
    `unsigned __int128` above its maximum) a wide literal holding the
    argument's exact constant (`temp.arg.nontype/{bit-int-wide,int128}-value.cpp`);
  - for a class-type NTTP, a `DeclRefExpr` to the `TemplateParamObject`
    (§6.10);
  - for a reference or pointer NTTP, the entity expression;
  - for a floating-point NTTP, a `FloatingLiteral` of the value (WP-404;
    tests/conformance/temp.arg.nontype/float-argument-substituted.cpp);
  - for a canonical value-dependent argument (`ArgView::Profile`, a
    dependent default argument met in a partial substitution), its first
    expression, as `template_arg` uses it (WP-404).
  This matches clang's dump.
- **`ConceptSpecializationExpr`, `RequiresExpr`.** §8.
- **Lambdas** get a new closure class per instantiation (WP-075).
- **`SizeOfPackExpr`, folds, expansions.** §7.
- **Braced lists** (`Substituter::init_list`, WP-192). A dependent list
  is rebuilt clause by clause and re-run through the initialization
  action. A semantic list that has a written form (`syntactic_list`,
  spec/ast.md §5.6) is rebuilt from that form, as clang's
  `TransformInitListExpr` uses the syntactic form. Any other semantic
  list is rebuilt from its clauses without the trailing fillers
  (`ImplicitValueInitExpr`, `CXXDefaultInitExpr`) its check appended,
  which clang's syntactic form never holds (WP-613, spec/sema.md §6). A
  `DesignatedInitExpr` gives its designators back, with array indices
  substituted. A `PackExpansionExpr` of a `DesignatedInitExpr` expands to
  one designated clause per element. Designated lists in default member
  initializers, default arguments (`T t = {.y = 2}`, kept dependent when
  the parameter type is), NTTP arguments and requirements are handled
  the same way. An unknown field in a requirement is a substitution
  failure. The aggregate checks of spec/sema.md §6 run on the
  instantiated type. `T{x}` of an aggregate whose temporary is bound
  (`CXXBindTemporaryExpr` between the functional cast and its list) is
  rebuilt from the list the same way (WP-404;
  tests/conformance/expr.type.conv/aggregate-list-temporary-substituted.cpp).
- **Analyzed initializations as operands** (WP-296). An operand that is
  only instantiation-dependent (it names a local of the template) was
  analyzed at definition; its resolved initialization is not reused but
  formed again from the substituted operands (clang
  `TransformInitializer` reverting to the syntactic form):
  - a parenthesized aggregate initialization (`CXXParenListInitExpr`,
    [dcl.init.general]/16.6.2.2), alone or under its no-op functional
    cast, becomes `act_on_cast` (one operand) or `act_on_paren_list_cast`
    of the substituted type from its explicit initializers
    ([expr.type.conv]/2);
  - a braced list behind implicit nodes (`Substituter::braced_source`:
    an `InitListExpr`, the backing array of a `CXXStdInitializerListExpr`
    or a list-initializing `CXXConstructExpr`) is that list again: a
    braced call argument (also of an `operator()` call), the right
    operand of an assignment operator function (`act_on_assign_list`,
    [expr.assign]/8), a returned list and a for-range-initializer;
    elsewhere a `CXXStdInitializerListExpr` list-initializes its
    substituted `std::initializer_list` type ([dcl.init.list]/5).
  - (WP-301) a nested element of a braced list, a braced argument of a
    paren constructor call (`P p({a, 1}, b)`), and a braced argument of a
    member, `operator()` or `operator[]` call are rebuilt as lists by
    `call_arg_list`. That function stops at the first `CXXDefaultArgExpr`.
    The written arguments of a mem-initializer or parenthesized initializer
    (`written_args`) take the same rebuild (`braced_args`) when any argument
    is braced, so `: m(v, {v + 1, v + 2})` and `: m(a..., {a..., 7})`
    instantiate their lists ([class.base.init]/7;
    `class.base.init/braced-arg-mem-init-template.cpp`, WP-445);
  - a list-initializing `CXXConstructExpr` operand is formed again
    through `init_from_list_at` of the substituted type
    ([dcl.init.list]/3.7). A single-argument non-list construct is its
    substituted argument. The list of an initializer-list constructor's
    construct (`ConstructInfo::std_init_list`, `construct_list`) is the
    braced list its one `std::initializer_list` argument was built from
    (an empty one for the value-initialized `std::initializer_list` of
    `V v{}`), not a list holding that argument (clang
    `TransformInitializer` unwraps `isStdInitListInitialization`). Rebuilt
    as `{{...}}`, a one-element list would be chosen by
    [over.match.list]/1.1, so `V<P> v = {{"a", "b"}, {"c", "d"}}` stays
    two elements and `V v{"2", 1}` is not `V v{{"2", 1}}` (WP-801, WP-804;
    `dcl.init.list/initializer-list-ctor-in-template-keeps-list-run.cpp`,
    `dcl.init.list/init-list-ctor-element-list-in-template.cpp`);
  - a compound literal `(T){...}` (C99 extension) is rebuilt from its
    peeled initializer as a list, and `act_on_compound_literal` runs
    again;
  - an analyzed `new` expression (one whose direct operands were not
    dependent, e.g. `new S[2]{{a}, {a, 1}}`) is formed again by
    `act_on_cxx_new` from the substituted placement arguments, the
    array bound as written, and the initializer as written: `(...)`
    stays a paren list, with its braced items kept braced (P0960
    `new S[3]({a, 1}, {a})`), and `{...}` stays a list. An implicit
    value-initialization is an empty initializer of the written style
    ([expr.new]/24).

  (WP-307) A non-list `CXXConstructExpr` operand with zero or two or
  more written arguments outside an initializer is direct-initialization
  done again ([dcl.init.general]/16.6.2; clang
  `TransformCXXConstructExpr`). Its arguments are substituted by
  `call_arg_list`, and `construct_class` of the substituted class runs
  with them. A zeroing construct with no arguments is
  value-initialization again (`value_init_class`,
  [dcl.init.general]/9). Test:
  `tests/conformance/temp.res/subst-construct-multi-arg.cpp`.
- (WP-301) GNU forms ([sema] §5.8):
  - a binary conditional `a ?: b` substitutes its common operand once
    and its false operand, then runs `act_on_binary_conditional` again
    (the common operand is evaluated once, [expr.cond] as extended);
  - a statement expression `({ ... })` opens `act_on_start_stmt_expr`,
    substitutes its compound body in a new block scope, re-runs
    `act_on_stmt_expr_result` on the substituted last expression
    statement (`Substituter::stmt_expr_result`) and finishes with
    `act_on_stmt_expr`. Because a statement expression in a dependent
    context is always value- and instantiation-dependent (below), its
    body is never shared with the pattern.

  Every other expression or statement kind without an arm reaches the
  generic catch-all, which gives the NYI diagnostic `substitution into
  <kind>` and never panics. A census over the libc++ 19 10% sample
  (790 tests, `-fsyntax-only`) found no kind reaching it; the kinds left
  are unreachable today because their construct is rejected or
  unimplemented before substitution: coroutine expressions and
  statements, `template for` and computed `goto`. A `contract_assert`
  (`ContractAssertStmt`) has its own arm since WP-706, and a function's
  `pre`/`post` specifiers are substituted with the function
  (`templ/subst/contracts.rs`; spec/contracts.md §5, Templates).
- **`ChooseExpr`** (WP-307; clang `TransformChooseExpr`): the condition
  (unwrapped from its `ConstantExpr`) and both operands are substituted
  and `act_on_choose_expr` runs again.
- **Dependent `OffsetOfExpr`** (WP-307; clang `TransformOffsetOfExpr`):
  the stored operand type and the index expressions are substituted and
  `act_on_builtin_offsetof` runs again with the stored components, so
  every check of sema.md §5.8 happens at instantiation. A
  non-dependent `OffsetOfExpr` (no pattern) is kept as is.
- **`ExtVectorElementExpr`** (WP-307; clang
  `TransformExtVectorElementExpr`): the base is substituted and
  `act_on_member` runs again with the stored accessor, so the lanes and
  the result type are recomputed (sema.md §5.19).

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
- **A failed statement invalidates the specialization** (clang 19's
  `TransformCompoundStmt` `SubStmtInvalid` → `InstantiateFunctionDefinition`
  `setInvalidDecl`; WP-145). When a statement of a function body fails to
  substitute *and reported a new error while doing so*, the function
  template specialization is marked `INVALID`. Consequences:
  - a later `deduce_call` that forms the same specialization reports
    `candidate template ignored: substitution failure [with ...]`
    (`get_function_spec` returns `Err` for an invalid spec);
  - a placeholder (`auto`) return type left undeduced by the failed body
    deduces to the error type, not `void`, so no "cannot be used before
    it is defined"/void cascade follows.
  The exceptions match clang, which keeps the statement:
  - a `DeclStmt` whose variable *initializer* failed (the variable is
    invalid, the statement is not; `Substituter::init_failures`);
  - a statement that only refers to an already-invalid entity (it fails
    without a new error: clang builds a `RecoveryExpr`).

  **A `return` with an erroneous operand in a placeholder-return
  function** (WP-434). A `return` whose operand fails without a new
  error (it names a local whose initializer failed, `auto l = t.foo();
  return l;`, or, below `-fclang-compat=22`, it is an error of the
  pattern) still invalidates the specialization when the declared return
  type contains a placeholder, whether or not an earlier `return`
  already deduced it. clang's `DeduceAutoType` answers
  `AlreadyDiagnosed` for an operand that `containsErrors()`, and
  `BuildReturnStmt` then marks the function invalid. So the type is not
  deduced `void` from what is left, the caller gets no `void` cascade,
  and a later call is a substitution failure. Inside a lambda body the
  lambda-expression fails instead (`Substituter::lambda_return_failed`).
  The enclosing statement then fails by the rules above. A
  non-placeholder return type is unaffected (one error, no
  invalidation). From clang 22 on, a `return` whose operand is an error
  of the pattern is dropped instead (`Substituter::stmt`), so a generic lambda
  `[](auto x) { return x + undeclared; }` deduces `void` and each call
  cascades, as in clang 22. A non-generic lambda outside any template
  keeps clang 19's single error under both levels: clang 22 also
  deduces `void` there, but kiln does not follow it. Tests
  `dcl.spec.auto/return-of-failed-local.cpp` (both releases) and
  `expr.prim.lambda/undeclared-identifier-no-void-cascade.cpp`
  (`clang<22`).

  **A lambda body that reports its own error** makes the whole
  lambda-expression fail (clang `TransformLambdaExpr` returns
  `ExprError`; WP-163). Errors that belong to a nested instantiation the
  body triggered do not count (`inst.body_errors`). Consequences:
  - The closure type is discarded (`act_on_lambda_error`), so no cascade
    follows from a closure whose call operator returns `void`. Neither a
    `return l();` nor an NSDMI `int m = [] { ... }();` gets one.
  - The enclosing statement fails by the rule above. An expression
    statement or a `return` invalidates the specialization. A `DeclStmt`
    initializer does not invalidate it; only the variable is invalid.

  Test `expr.prim.lambda/lambda-body-substitution-failure.cpp`.
- **Labels and `goto`** (WP-153). A `LabelStmt`, a `GotoStmt` and the
  labels of an `asm goto` are rebuilt by name (`act_on_label`,
  `act_on_goto`, `act_on_asm_goto_label`), so each instantiation owns its
  `LabelDecl`s and the jump-scope checker (spec/sema.md §7.4) runs on it.
- **Try-blocks** (WP-160, clang `TransformCXXTryStmt`/`TransformCXXCatchStmt`).
  The compound statement is substituted in a try scope. Each handler gets
  its control scope, and its exception declaration is declared again from
  the substituted type through `act_on_exception_decl`. That repeats the
  decay, the incomplete-type and abstract-type checks and the
  copy-initialization from the exception object, and maps the pattern
  variable to the new one. `act_on_try` then checks the handler order on
  the instantiated types; -Wexceptions is `allow_duplicate`, so it prints
  once per instantiation as in clang. A pattern handler whose declaration
  is invalid fails the statement. Function-try-blocks take the same path
  (`body` of a non-compound body).
- **`[[assume]]`** (WP-160): an `AttributedStmt`'s assumptions are
  substituted, converted and checked again (`build_cxx_assume_expr`,
  -Wassume at instantiation). Its other attributes (`likely`,
  `unlikely`, `fallthrough`; WP-182) are kept as they are.

### 5.6 Declarations

- **Order.** The function signature is substituted in lexical order
  ([temp.deduct.general]/7): a leading return type, then the parameters,
  then a trailing return type, then the trailing requires-clause (§8.3).
  The exception spec and `explicit(bool)` come later, when needed
  ([temp.inst]/14).
- **Parameters.** `ParmVar`s are substituted. A function parameter pack
  becomes N `ParmVar`s (`DeclOrPack::Pack`). A specialization declared
  from an earlier declaration takes the definition's parameter names
  when its body is instantiated (clang instantiates them from the
  definition, `addInstantiatedParametersToScope`, [temp.inst]/4), so a
  local of the body may reuse a forward declaration's parameter name.
  This holds for function template specializations and for members and
  member templates of class templates defined out of class, parameter
  packs included (`rename_params_from_pattern`, WP-295;
  `tests/conformance/temp.inst/member-definition-parameter-names.cpp`).
- **Variables** keep the pattern's form of initialization (`cinit`,
  `callinit`, `listinit`): `T v{a, b}` stays direct-list-initialization,
  which may choose an explicit constructor. A braced list is redone as a
  list even when the pattern's full-expression (`ExprWithCleanups`) wraps
  it, as when its elements' constructors bind temporaries: `T a[] = {{1}}`
  with `T(const int&)` (WP-278;
  `tests/conformance/dcl.init.aggr/array-of-class-from-list-in-template.cpp`).
  A construction is redone from its written arguments (clang
  `TransformInitializer`), also when it was resolved at definition: `T x(e)` and `T x{e}` with `e` of class `T` or a
  class derived from it ([dcl.init.general]/16.6.2, [dcl.init.list]/3.2)
  call the copy or move constructor at instantiation. Only a construction
  with no parentheses and no written argument is a default-initialization.
  A braced-init-list argument of a dependent parenthesized initializer
  (`C c({1, 2}, a)`, `new C({1, 2}, a)`) stays a braced argument when
  the list is substituted, so the constructor's parameter is
  initialized from it by [over.ics.list] (`call_arg_list`; WP-295,
  `tests/conformance/over.ics.list/dependent-paren-init-braced-arg.cpp`).
- **Block-scope namespace aliases** ([namespace.alias]; clang
  `VisitNamespaceAliasDecl`): the target is never dependent, so the
  alias is declared again in the instantiation's block scope with the
  same target, or the instantiation's alias when the target is an
  earlier local alias (`subst/stmts.rs` `local_decl`; WP-370, libstdc++
  variant:1546,
  `tests/conformance/namespace.alias/local-alias-in-function-template.cpp`).
- **Local classes** inside an instantiated body are instantiated fully
  and immediately ([temp.inst]/3), by the body's own substituter
  (`subst/local.rs`, WP-131): so their members see the function's
  template arguments and its local declarations. Member classes are
  defined with the outermost local class. Member function bodies are
  defined after it is complete, in the re-entered class scopes, as the
  parser replays them. Each body is a function of its own: the enclosing
  function's references still waiting for the end of their
  full-expression (odr-use marking, potential lambda captures, as a
  tuple-like binding's `get` call leaves them) are set aside while it is
  instantiated, so the body never claims them as references to an
  enclosing local (WP-789,
  `tests/conformance/class.local/member-body-after-tuple-binding-run.cpp`).
  Local enumerations are substituted the same way.
  An anonymous struct or union member of a local class is instantiated as
  in a class template (`unnamed_member_class`), which declares its member
  object and injects its members' names (WP-192). A block-scope
  anonymous union of the body (of a function template, a lambda or a
  generic lambda in one) is defined by its declaration statement's class
  (`local_class`, marked anonymous), then its unnamed object, the
  statement's next declaration, is declared again for it
  (`local_anonymous_object`), injecting the members' names; the pattern
  names a member as `obj.member`, so only the object is linked (WP-311,
  `tests/conformance/class.union.anon/function-template-locals.cpp`,
  `template-unnamed-members-ast.cpp`).
  The class body is instantiated under a `MemberClass` frame and each
  member function body under a `MemberFunction` frame at the outermost
  local class (clang `InstantiateClass` / `InstantiateClassMembers`,
  DR1484), so an error inside gets "in instantiation of member class 'L'
  requested here" / "member function 'L::m' ..."; a nested local class is
  named through its function (`f()::L::N`, `local_class_decl_name`). A
  local declaration whose statement failed (a typedef or using of a
  failed type, a variable, a local class made invalid by a member) stays
  invalid in `failed_locals`: its later uses fail silently instead of
  reporting a second error (WP-316; was "not yet implemented: reference
  to a member of a dependent context").
  A block-scope function declaration of the body, including an operator,
  is declared again in the instantiation's block with the substituted
  signature (`local_decl`, `function_decl` with `DeclaratorContext::Block`,
  WP-329; [basic.scope.block], [temp.inst]/3), so it redeclares and
  conflicts like a non-template one. A substituted declaration that differs
  from a namespace function only in its return type is diagnosed for each
  instantiation, with its instantiation note. A local class's friends are
  checked against the block once, at the template definition, even when
  their types are dependent (`friend void ft(T);` matches `void ft(T);`):
  instantiation repeats no local friend error (`tests/conformance/temp.inst/local-function-declaration.cpp`,
  `local-function-declaration-run.cpp`). A local (or member) class
  instantiated from a template keeps its implicit special members when a
  member made it invalid, as a specialization does (`ensure_implicit`,
  clang `CanDeclareSpecialMemberFunction`), so `L()` adds no "no
  matching constructor".
  Tests: `temp.inst/failed-local-declaration-uses.cpp`,
  `temp.deduct/failed-local-parameter-sfinae.cpp` (a failed parameter of
  a requires-expression or of a deduced function is SFINAE-silent).
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
- Warnings and remarks are held back in `SfinaeState.warnings` (WP-392).
  What counts is the diagnostic's class, not its mapped severity: a
  warning that `-Werror` turns into an error is held back like any
  warning and does not fail the substitution (clang's SFINAE response
  is per diagnostic id; WP-418,
  `tests/conformance/temp.deduct.general/werror-warning-not-substitution-failure.cpp`).
  A warning whose default mapping is an error still fails it.
  clang's `SFINAEFailure` warnings fail it whatever their mapping
  (`is_sfinae_failure`): the designated-initializer order/override
  ExtWarns (spec/sema.md) and the narrowing ones, `-Wc++11-narrowing`
  in a list-initialization or a converted constant expression
  ([dcl.init.list]/3.7, [expr.const]), so a trait probing `T{v}` under
  `#pragma GCC diagnostic ignored "-Wnarrowing"` or `-Wno-narrowing`
  still sees the substitution fail (WP-685,
  `tests/conformance/temp.deduct.general/narrowing-ignored-still-substitution-failure.cpp`).
  When function template deduction forms a specialization, the ones its
  signature substitution produced move to
  `TemplState.suppressed_diags: FxHashMap<DeclId, Vec<Diag>>`, keyed by
  the specialization. `Sema::emit_suppressed_diags` reports them when
  that specialization is first used (from `diagnose_use_of_decl`), once.
  This is clang's `SuppressedDiagnostics`. clang 22 keeps the context
  notes ("in instantiation of function template specialization …"
  pointing at the point of use; `TemplState.deduce_poi` is a qualified
  callee's name, where clang marks it referenced); clang 19 replays the
  bare warning, so the notes are dropped under `-fkiln-compat=19`.
  A failed deduction drops them. One deduction collects the warnings of
  all its phases (clang's single `TemplateDeductionInfo`): `deduce_spec`
  appends what each SFINAE frame held (`in_sfinae_held`) to one list, so
  the explicit-argument substitution's come before the signature's.
  For a target type, and for a template-id naming one specialization
  without a call (`&f<volatile int>` with `auto`, `decltype`), the
  explicit phase substitutes the whole function type. clang
  `BuildFunctionType` reports that at the template's name. In clang 22
  its note is "while substituting explicitly-specified template
  arguments into function template 'f' ", with an empty argument list,
  so it ends in a space. `keep_deduction_warnings` stores the list only
  on success. If the specialization already has an entry, the list is
  stored only when the specialization was formed by this deduction;
  otherwise it is dropped. clang stores only into an empty slot. An
  emitted entry stays, empty, so a second `&n<volatile int>` or a later
  call reports nothing. Test `depr.volatile.type/volatile-return-funcaddr{,-19}.cpp`.
  The parser resolves a template-id that is not called to its
  specialization before the target type is known
  (`resolve_single_template_id`), where clang keeps it an overload set.
  So that resolution takes the specialization's held warnings out of
  their entry and parks them in `SpecialState.pending_tid_warnings`,
  keyed by the reference. The entry follows the enclosing parentheses
  and `&` (`widen_pending_template_id`). The warnings are reported at a
  call of the reference, or at the end of the full-expression or
  declaration (`flush_pending_deleted`). An initialization or `return`
  of a pointer or reference to function whose type the specialization
  does not have finds no match ([over.over]/1): `conv`'s
  `template_id_no_match` reports "address of overloaded function 'f'
  does not match required type 'int ()'" at the name, with the
  deduction notes of the overload-set complaint (`addr_no_match_notes`).
  It puts the warnings back into the entry for the specialization's
  first use (`drop_template_id_warnings`). Inside SFINAE nothing is
  parked. clang stores a deduction's warnings only when that deduction
  succeeds, so from clang 22 on their context notes name the later use:
  the drop records the template-id's location
  (`SpecialState.dropped_tid_warnings`), and the next take of the entry
  (`take_suppressed_diags`, from `resolve_single_template_id` and
  `emit_suppressed_diags`) moves the notes there to the use's location
  (`Diag::relocate_notes`; WP-424). Only the signature substitution's
  warnings go back: `keep_deduction_warnings` records how many of the
  entry's lead it as the forming deduction's own (explicit arguments,
  defaults; `TemplState.deduction_diags`), and the drop strips that
  many. The next deduction of the specialization then treats the
  handed-back entry as clang's empty slot and puts its own phases'
  warnings ahead of it, so an address-of reports its explicit-argument
  warning again while a call, whose explicit arguments are substituted
  into the parameter types alone, reports none (WP-444). Tests
  `over.over/template-id-no-match{,-19,-restored,-call,-call-19}.cpp`.
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

**Immediate context (P4149R1, Croydon DR; WP-757).** [temp.deduct.general]/8
now defines the immediate context as the substitution itself, excluding
any instantiation it triggers and any lambda body, default argument,
default member initializer or noexcept-specifier reached through it.
kiln realises the boundary cases with frames:
- `FrameKind::DefaultFunctionArg` is `Some(false)`. Substituting a local
  class's or closure's member function declaration
  (`Substituter::default_args`, `templ/subst/decls.rs`) pushes one such
  frame per default argument at that argument's location, so an error in
  it is a hard error with clang's "in instantiation of default function
  argument expression for 'f<X>'" note, even under a deduction.
- A lambda's noexcept-specifier that is value-dependent is substituted
  under an `ExceptionSpec` frame (`Some(false)`) when the stack is in
  SFINAE mode (`templ/subst/lambda.rs`), so an ill-formed one is an error
  and not a deduction failure. clang 22 still treats it as a substitution
  failure; outside SFINAE nothing changes.
- A lambda body, a default member initializer and a class template
  definition were already `Some(false)` frames (`LambdaSubstitution`,
  `Nsdmi`, `TemplateInstantiation`); substitution failures in the
  function type, template parameter types and alias templates stay
  SFINAE.
- **Template heads and lambda bodies** ([temp.pre]/2): a declaration in a
  lambda body takes none of the template heads outside the lambda; those
  await the declaration whose declarator holds the lambda (`template<class
  T> auto f() -> decltype([] { int t; return t; }())`).
  `Sema::lambda_head_floor` returns the head count recorded when the
  innermost open lambda body began, and `take_template_head` stops below
  it. Only lambdas from `lambda_base` up count: `in_inst_context` (the
  point-of-instantiation switch, `templ/class.rs`) sets `lambda_base` to
  `lambdas.len()` for its duration, because it clears the heads but not
  the lambda stack, and a lambda around the point of instantiation must
  not floor the instantiated declaration's heads.
- Tests: `tests/conformance/temp.deduct.general/p4149-*.cpp`,
  `tests/conformance/temp.pre/lambda-local-decl-in-declarator.cpp`.

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

`Tdk` mirrors clang's `TemplateDeductionResult`. `Tdk::Invalid` is
deduction against an invalid function template (`deduce_spec`, WP-308):
the template is no candidate of a call (`add_template_candidate`), of
an address (`overload/address.rs`) or of an explicit specialization
(`match_function_spec_in`), and is never noted, as clang skips
`TemplateDeductionResult::Invalid` candidates.

**Memoized specializations** (WP-303): a function declaration
specialization (`get_function_spec`) or an alias template substitution
(`alias_template_type`) formed while the SFINAE trap captured an error
(`InstStack::sfinae_errors()` grew) is a substitution failure, not a
valid cached entry: without this, the first caller in SFINAE mode
cached e.g. a narrowing `_Ap[] = {std::forward<_Up>(u)}` signature as
valid and later overload sets saw a viable candidate
([temp.deduct.general]/8; libc++ `variant`'s `__overload`).

**Memory.** Types and nodes that a failed substitution created are
retained, since the AST is append-only. They are metered as
`templ.sfinae_discarded` (§11.3).

#### 5.7.1 `enable_if` failures (clang 19, WP-274)

clang words a missing `type` in `enable_if` specially. This changes the
text and location only, never whether the failure is SFINAE; the
mechanism above is the same for every name. kiln matches clang:

- **Recognition** (`Substituter::enable_if_failure`, clang `isEnableIf`):
  the name is `type` and its qualifier substitutes to a specialization
  of a class template named `enable_if`. The *condition* is its first
  argument when that is still an expression which, after parens and
  casts, is not a `bool` literal.
- **With a condition**: `failed requirement 'C'; 'enable_if' cannot be
  used to disable this declaration` at the failed conjunct (`C` is the
  first false operand of a `&&` chain, `failed_condition`, printed as in
  `static_assert`).
- **Without one** (the argument is already a value, as through an alias,
  or a literal): `no type named 'type' in 'enable_if<false, int>';
  'enable_if' cannot be used to disable this declaration` at the written
  first argument (in an alias: the alias pattern's argument, since alias
  substitution does not carry written locations, §4.10).
- **`enable_if_t` rewrite** (`alias_template_type`, clang
  `CheckTemplateIdType`): when substituting an alias named `enable_if_t`
  or `__enable_if_t` fails in a SFINAE context and the captured first
  error is the "without a condition" form, it is replaced by the "with a
  condition" form for the alias's own first argument, kept at the old
  location. Here a literal is not skipped (`requirement 'false'`).
- **Notes** (`deduction_note`, clang `DiagnoseBadDeduction`): the "with a
  condition" error gives `candidate template ignored: requirement 'C'
  was not satisfied [with ...]` at the template; the "without" error
  gives `candidate template ignored: disabled by 'enable_if' [with ...]`
  at the error's location. Every other substitution failure keeps
  `candidate template ignored: substitution failure [with ...]: <error>`.

| Form (P = int) | Note |
| --- | --- |
| `typename enable_if<is_ptr<P>::value && B, int>::type` | requirement 'is_ptr<int>::value' |
| `enable_if_t<is_ptr<P>::value, int>`, also via another alias | requirement 'is_ptr<int>::value' |
| `enable_if_t<false, T>` | requirement 'false' |
| `my_t<is_ptr<P>::value, int>` (alias not named `enable_if_t`) | disabled by 'enable_if', at `my_t`'s pattern |
| `typename enable_if<B, int>::type` with NTTP `B` | disabled by 'enable_if', at `B` |

#### 5.7.2 Invalid substituted parameter types (WP-306)

[temp.deduct.general]/11 lists the invalid types whose formation during
substitution is a deduction failure. For a function parameter
(`Substituter::param_type`, used by `param_infos` and by the explicit
argument substitution in `deduce_call`):

- **`void`** (`T` with `T = void`, a pack element `void` of `Ts...`):
  `argument may not have 'void' type` at the substituted type, clang's
  `err_param_with_void_type`; a SFINAE failure with the usual
  `substitution failure [with ...]` note. A function type substituted
  to have a `void` parameter (`void (*)(T)`) fails the same way; outside
  deduction (an alias `template<class T> using F = void(T);` used as
  `F<void>`, a member `void f(T)` of `W<void>`) it is a hard error at the
  alias's or member's parameter, with the instantiation note.
- **Parameter adjustment** ([dcl.fct]/5): every parameter of a substituted
  function type is adjusted after substitution exactly as a written one
  (`adjust_param_type`: array of `T` to pointer to `T`, function to
  pointer to function, top-level cv dropped), so `F<int[3]>` is
  `void(int*)` and `F<int()>` is `void(int(*)())` (clang
  `BuildFunctionType`; test
  `temp.alias/function-type-parameter-adjustment.cpp`, WP-311).
- **Array of `void`, of a function or of a reference**: clang's
  `BuildArrayType` wording, `array has incomplete element type 'void'`
  (also for a non-template `void x[3]`, which no longer uses the old
  `declared as array of void` text).
- **Array of an abstract class** (as clang 19, which has not implemented
  P0929): `array of abstract class type 'A'`, from
  `require_non_abstract_element`.
- **Pointer to reference, reference to `void`, function returning an
  array or function**: already failures of `Substituter::ty`.

After the checks the parameter type is adjusted ([dcl.fct]/5: array to
pointer, function to pointer, `Sema::adjust_param_type`); the declared
type is kept as `ParmVar::original_ty` (and `ParamInfo::original_ty`), as
clang's `getOriginalType`, for a written parameter and for a substituted
one whose declared type is still dependent (a member template of an
instantiated class template, so that `T a[3]` with `T = void` still fails
at its own deduction). A fully substituted parameter has none: a
range-`for` over it in an instantiation is clang's plain invalid-range
error, not the array-parameter diagnostic of the definition.
`tests/conformance/temp.deduct.general/invalid-parameter-type-sfinae.cpp`
(the libc++ `MoveOnly` deleted `operator,` shape),
`.../invalid-parameter-type-notes.cpp` (strict notes) and
`.../invalid-parameter-type-member-template.cpp`.

### 5.8 Written locations of dependent names

Kiln's types are interned and carry no `TypeLoc`, so the substituter
only knows an anchor `loc` (the pattern declarator or the use). To report
errors found after substitution where clang does, at the name as written,
`templ.written_locs` (`templ/written.rs`) maps a dependent type or NNS
(`WrittenKey::Type`/`Nns`) to the sorted locations where the parser
formed it:

- `act_dependent_name_at` (`T::type`, `T::template Z<int>` written with
  `typename`), `act_on_nested_name_component` (a dependent `T::`/`T::N::`
  prefix) and `act_template_id` (a dependent template-id) record the name's
  location.
- `act_on_nested_name_template_id` records a dependent `X<args>::`
  component (`WrittenKey::Nns`) at its start: the `template` keyword when
  one is written (`O<T>::template I<T>::`), else the template name (the
  last qualifier name, so `::N::H<T>::` records the `H`). Substituting
  the qualifier of `typename H<T>::type` completes `H<int>` with that
  location as the point of instantiation, so the "in instantiation of
  template class" note points there, as clang's
  `RequireCompleteDeclContext` does, not at the declarator (WP-307;
  `temp.inst/instantiation-note-template-id-column.cpp`).
- A using-declaration's qualifier error (`T` not a class or not a base,
  "cannot be used prior to '::'") is reported at the start of its
  nested-name-specifier, as clang does: `Substituter::qualifier_begin`
  walks the NNS prefixes to the outermost dependent one and returns its
  written location (block scope `subst/stmts.rs` `local_using_decl`,
  member `templ/class.rs` `member_using`/`member_using_typename`, pack
  expansions included; WP-328,
  `namespace.udecl/dependent-qualifier-location.cpp`). A non-dependent
  leading prefix (`N::X<T>::m`) or a leading `::` is not recorded, so
  such a qualifier reports at its first dependent component.
- `act_on_atomic_type` records a dependent `_Atomic(T)` specifier or
  `_Atomic T` qualifier (`WrittenKey::Type` of the atomic type) at its
  `_Atomic`: an operand that substitution makes invalid (array, function,
  reference, atomic, qualified, `_BitInt`) is reported there with the
  instantiation note, as clang's `TransformAtomicType` does, not at the
  declarator; an alias template reports at its `using` (`written_locs`
  off; WP-461, `ext.c11.atomic/atomic-specifier-subst-loc.cpp`).
- Recording happens only while parsing (the instantiation stack is
  empty), so instantiation never grows the table.
- `Substituter::written(key, loc)` returns the recorded location nearest
  `loc` (ties to the earlier), or `loc` when none is recorded. The
  DependentName, DependentTemplateSpecialization and TemplateSpecialization
  arms of `ty` and the identifier arm of `nns` use it for "no type named",
  "typename specifier refers to non-type member", "no member named",
  "cannot be used prior to '::'" and access errors. The notes are
  unchanged. A DependentTemplateSpecialization (`typename
  T::template X<int>`) substitutes its qualifier with the written NNS
  location as context, so an error in `T::` is reported at the `T`.
  A member alias-declaration gets its access (`set_member_access`), so
  an access check through it names the right member access.
- An interned type written at several places resolves by occurrence,
  not by distance (WP-214). While a declaration's type is substituted,
  `Substituter::written_scope` (a `WrittenScope`: anchor, side, bound)
  says which occurrence it wrote, and `written_loc_in` picks it.
  - *After* `bound`: the first occurrence at or after `bound`.
  - *Before* `bound`: the last occurrence at or before `bound`.
  - The scope applies only while the anchor matches. A type substituted
    at another anchor (a default argument, an initializer) falls back to
    the nearest occurrence.
  - Under an *after* scope, a dependent type substituted again takes
    its next occurrence (`Substituter::written_used`, WP-220), so `T::type
    (*m)(T::type)` reports both access errors where clang does. Types are
    substituted in written order (a leading return type, then the
    parameters). Nested-name-specifiers do not advance: one occurrence
    looks them up more than once.
- The rule each declaration uses (`subst/decls.rs`):
  - A parameter, field, variable, static data member or typedef whose
    decl-specifiers' start is recorded (`decls.cold.begin_loc`) uses
    *after* that start. This finds `int (*p)(T::type)`'s own occurrence,
    which is written after `p`. Variables, static data members and
    typedefs record it only when their type is dependent (WP-220), and
    an instantiation keeps it while its type is still dependent.
    Otherwise the rule is *before* the declarator-id, or before where an
    unnamed parameter's name would be.
  - A leading return type uses *before* the function's name.
  - An alias-declaration uses *after* its name.
  - A trailing return type uses *after* its `->`
    (`TemplState.trailing_arrow`, recorded when the trailing type is
    dependent; WP-220), so `auto f(int (*p)(T::type)) -> T::type` finds
    its own occurrence. A conversion function's type, and a trailing
    return type with no recorded `->`, use *after* the last parameter's
    location.
- Order. A leading return type is substituted before the parameters, so
  the first error reported is the first one written, as in clang's
  `TransformFunctionProtoType`. A trailing return type is substituted
  after them. The parameters still enter scope before the exception
  specification.
- Once per declarator. In `typedef T::type g, (*h)(T::type);` both `g`
  and `h` fail at the first `T::type`, and clang reports it twice. Kiln
  does too: the member being substituted salts the dedup context
  (§10.2, `member_salt`).
- Dependent names inside expressions (`T::x` in a default argument) are
  not affected. The expression keeps its own location, so these are
  exact without the table. A type operand of `sizeof`, `alignof` or
  `typeid` is a type, though: it uses *after* the keyword
  (`Substituter::ty_written_after`, WP-220), so `T::type f(int =
  sizeof(T::type))` reports the default argument's own occurrence, not
  the nearer return type's.
- `typename T::template Z<int>` checks the access of a member class or
  alias template when it is substituted ([class.access]/4; WP-220), as
  `typename T::type` does. A function template's access is checked by
  overload resolution. A partially
  substituted member template is instantiated from its original pattern
  with all levels of arguments at once (`MultiLevelArgs`), so its types
  keep their written keys. Types formed during instantiation therefore
  do not need recording.

## 6. Function templates

### 6.1 Pipeline for a call: `deduce_call(ft, explicit, args, ctx)`

1. **Explicit args** ([temp.arg.explicit]).
   - Check them against the leading parameters and convert them.
   - Substitute them into the function type under the
     `ExplicitTemplateArgumentSubstitution` frame. Parameters without
     explicit args stay parameters.
   - A trailing pack that was given explicitly can be *extended* by
     deduction (/9).
   - (WP-804) For a call, a return type written before the parameters
     is substituted too, first and only to check it (clang
     `SubstituteExplicitTemplateArguments`): with every argument given,
     `enable_if_t<C<A>, int> f(const S<A>&)` fails here, before the
     CWG1391 check converts an argument to `S<A>` and so instantiates it
     (`temp.deduct.call/explicit-args-return-sfinae-before-nondep-conversion.cpp`).
     A trailing return type may name the parameters and is left to
     step 4 (xfail
     `temp.deduct.call/explicit-args-trailing-return-sfinae-before-nondep-conversion.cpp`).
   - Failure gives `InvalidExplicitArguments` or `SubstitutionFailure`.
2. **Deduce from each (Pᵢ, Aᵢ) pair** ([temp.deduct.call]). Pᵢ is the
   parameter type; Aᵢ is the argument's type and value category.
   - **Adjustments, when P is not a reference.** A undergoes
     array-to-pointer and function-to-pointer decay, and its top-level cv
     is dropped. Top-level cv on P is dropped too: the written
     prototype that deduction reads keeps it (`void f(const T t)`
     deduces `T = int` from an `int`, with or without a trailing
     `requires requires { t; }`; WP-231).
   - **When P is a reference.** Use the referred type. A forwarding
     reference (`T&&`, where `T` is this template's own unqualified
     parameter) with an lvalue A uses A&. When the referred type is an
     array more cv-qualified than A, its qualifiers are intersected
     with A's before matching (clang's `getUnqualifiedArrayType` rule
     in `DeduceTemplateArgumentsByTypeMatch`; WP-207): `const T (&)[N]`
     against `int[3]` deduces `T = int`, and the "more qualified" check
     of /4.1 accepts the result (an array's cv is its element's).
   - **A braced list.** If P is `initializer_list<P'>` or `P'[N]`, deduce
     P' from each element (and N). Otherwise it is non-deduced.
   - **An overload set.** If exactly one member deduces, use it (/6). If
     the set contains a template, it is non-deduced.
   - **A trailing function parameter pack.** Match every remaining A
     against the pattern. A pack that is not last is non-deduced.
   - **A P without template params.** It is not deduced. Its conversion
     is checked after deduction (CWG1391) and gives
     `NonDependentConversionFailure`. The same holds for a P whose
     template params all sit in non-deduced contexts (`FS<A...>`, an
     alias of `BFS<char, typename ti<A>::type...>`): a failed match of
     its argument is not a deduction failure, and a successful one is
     not an original argument for /4. A pack expanded in P's template
     argument list with a deducible pattern (`Tup<int, Us...>`) is in a
     deduced context, so a mismatch there still fails deduction. A
     non-type expansion is deducible only when its pattern is a bare
     template parameter of the deduced depth (`Spec<N...>`); any other
     expression pattern (`Spec<conv<Args>()...>`, `Spec<sizeof(Args)...>`,
     directly or through an alias) is a non-deduced context
     ([temp.deduct.type]/5.3), so `Args` comes from the trailing pack and
     the argument converts to the substituted P, through a consteval
     converting constructor if need be (WP-566; tests
     `temp.deduct.type/nttp-pack-expansion-non-deduced-*.cpp`).
3. **Finish** ([temp.deduct.general]/5):
   - an undeduced parameter takes its default (substituted in order); a
     pack takes the empty pack; anything else gives `Incomplete`;
   - check the argument of each P without deduced template params
     (step 2's last bullet) for an implicit conversion, with the
     options `add_candidate` would use (`deduce_call`'s `opts`), before
     anything is substituted into the function type ([temp.deduct.call]
     CWG1391, clang `CheckNonDependent`). A bad one fails with
     `NonDependentConversionFailure` carrying the argument index and
     its sequence; `add_template_candidate` reports it as the
     template's `BadConversion` (no `[with ...]`, as before). A later
     parameter whose substitution would be a hard error (libc++
     `shared_ptr(nullptr_t, D, enable_if_t<__well_formed_deleter<D,
     nullptr_t>>)` against an `int*` argument) is thus never
     substituted (WP-296);
   - substitute everything into the function type under
     `DeducedTemplateArgumentSubstitution`. A failure is
     `SubstitutionFailure`;
   - only then check the associated constraints (§8,
     [temp.deduct.general]/5: "If the function template has associated
     constraints, those constraints are checked for satisfaction").
     Failure is `ConstraintsNotSatisfied`. The order is clang 19's
     `FinishTemplateArgumentDeduction` (Q12, CWG2369): the CWG1391 check
     above, then substitution, then constraints. So a candidate whose
     substitution fails *and* whose constraints are unsatisfied gets the
     substitution-failure note. A substitution that is a hard error
     (instantiating a class whose `static_assert` fails) is diagnosed
     even when the constraints would not be satisfied (WP-301,
     `tests/conformance/temp.deduct/substitution-failure-before-constraints.cpp`);
   - check that the deduced A is compatible ([temp.deduct.call]/4:
     equal, more cv-qualified, qualification-convertible, or a derived
     class). Failure is `DeducedMismatch`. kiln records, for each
     parameter with a deducible template parameter (probed by
     deducing P from itself, clang's `hasDeducibleTemplateParameters`)
     and an argument that is not a braced list or overload set, the
     pair (P, adjusted A); after substitution it compares the
     specialization's parameter type with A (`original_arg_mismatch`,
     WP-161). Each element of the trailing function parameter pack is
     such a pair with the pattern as P, except the explicit elements
     of a pack the pattern alone names (§6.4). The note prints the
     reference-stripped deduced type, and pointer-stripped only when A
     points to a class (/4.3), as clang does, as a template type diff
     pair (`'pr<[...], char>'`, `Sema::ty_diff`). This check follows the
     constraint check, as clang's `CheckOriginalCallArgDeduction`
     follows it.
   - /4.2 (WP-294): every call argument deduces with
     `TdfFlags::ALLOW_COMPATIBLE_FUNCTION_TYPE`, kept through pointers
     and member pointers (not into a function type's return or
     parameter types). A function type `P` then matches `A` regardless
     of a non-deduced `noexcept` (clang's type match ignores it;
     `noexcept(b)` with a deducible `b` is still deduced), and the
     deduced A check accepts a function pointer conversion of a pointer
     or member pointer, or a function lvalue whose type differs only by
     `noexcept`. Adding `noexcept`, or a mismatch below a second pointer,
     fails that check ("deduced type 'void (**)(int)' of 1st parameter
     does not match adjusted type 'void (**)(int) noexcept' of
     argument"). libstdc++'s `__gnu_cxx::__stoa(&std::wcstol, ...)`
     relies on it
     (`tests/conformance/temp.deduct.call/noexcept-function-pointer-argument.cpp`,
     `libstdcxx-deduce-noexcept-fnptr.cpp`).
     Binding `void (&)(int)` to a `noexcept` function after deduction is
     reference-compatible ([dcl.init.ref]/4, see spec/sema-conv.md).
4. **Success.** `get_function_spec(ft, canonical)` memoizes and returns
   the `FunctionTemplateSpecialization` decl. It has the substituted
   type and is a declaration only.

S3O ranks the result. The **body** of the winning candidate is
instantiated by `mark_referenced` when the call is built (§6.5).

**Shortcuts that change no outcome (WP-657).** Each one answers exactly
as the full pipeline would: the same `Tdk`, the same note, the same
frames and instantiations. `Sema::full_deduction` (doc-hidden, set from
`ParseOptions::full_deduction`) turns all of them off, so tests can
compare the two paths (`crates/kiln_parse/tests/deduction_prefilter.rs`).
- **Call pre-filter.** Before `add_template_candidate` builds the
  argument lists, `deduce_call_prefail` runs the checks that come before
  step 1 and need neither substitution nor allocation. These are an
  invalid template and an argument count below the minimum
  (`TooFewArguments`). On a failure, the candidate is added as the full
  path would add it. The `DeductionFailure` for `TooFewArguments` is
  formed only when `add_candidate`'s own arity check leaves the pattern
  viable, which is the only case where it is kept.
- **Substituted integer NTTP.** In step 1, an explicit non-type argument
  that is a `SubstNonTypeTemplateParmExpr` over an integer literal is
  converted without evaluation (`substituted_int_nttp_arg`). This
  requires that the literal's type and the parameter's type are the same
  builtin integral type (not an enumeration), that the value fits, and
  that no evaluation side state is pending.
  `Expr` → `TemplateArg::value` is then the value the evaluation
  would have produced.
- **Explicit-argument memo (WP-661).** Step 1's substitution of the
  explicit arguments into the parameter types (and, for an address or a
  declaration, the whole function type) is memoized per
  `(template, depth, explicit argument list, whole type, parameter
  types)` (`templ/explicit_memo.rs`). The
  `ExplicitTemplateArgumentSubstitution` frame is still pushed and popped
  on every attempt, so the instantiation report keeps every row with the
  same counts, nodes and bytes; only the work inside the frame is
  skipped. A result is kept only when the substitution succeeded with no
  SFINAE error, held back or emitted no diagnostic, created no AST node
  and no bytes, and pushed no instantiation frame of its own. A
  template declared in a local scope is never kept. A failure is never
  kept, so every failure's note and diagnostic is produced again at the
  same point, once per attempt. Such a kept substitution only formed
  types from types that already existed, so a repeat can answer no
  differently: completing a class it looked into would have created
  nodes. The memo is direct-mapped (2,048 slots, allocated on the first
  kept result, up to four parameter types per slot), so its memory is
  bounded (112 KiB: 2,048 slots of 56 bytes) whatever the TU size.

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

`cv T` against a function type `A` is `NonDeducedMismatch` (CWG1584;
clang's `DeduceTemplateArgumentsByTypeMatch`), even under
`IgnoreQualifiers`: `const T*` does not deduce from `void (*)()` and a
`template<class T> struct X<const T>` partial specialization does not
match `X<void()>`; `const T&` binding a function is unaffected (the
reference strips `P`'s qualifiers first). WP-418,
`tests/conformance/temp.deduct.type/cwg1584-cv-function.cpp` (libc++
`std::is_within_lifetime(const T*)` rejects function pointers).

`TopLevelParameterTypeList` ([temp.deduct.type]/10) is set for the
function type deduced against an address target or a declaration
([temp.deduct.funcaddr], [temp.deduct.decl]) and reaches only that type's
own parameter-type-list: there a forwarding reference `T&&` (or the
pattern `Args&&` of a pack) paired with an lvalue reference `U&` is
adjusted to `T`, deducing `T = U&` (`int (*)(int&) = f` for
`template <class... Args> int f(Args&&...)`). WP-446,
`tests/conformance/temp.deduct.funcaddr/forwarding-reference-{lvalue,conflict}.cpp`.

In a function type's parameter-type-list, a `P` that is not a pack
expansion paired with an `A` that is one (partial ordering: `A` came from
a function parameter pack) is `MiscellaneousDeductionFailure`
([temp.deduct.type]/10; clang's type-list `PackExpansionType` check), so
`R (C::*)(A) const` and `void (*)(A)` are more specialized than their
`(A...)` forms instead of ambiguous. WP-680,
`tests/conformance/temp.deduct.type/function-param-pack-vs-single-ordering.cpp`.

A class template-id P with a class A tries A first. If that fails and
`DerivedClass` is set, it tries each base of A
([temp.deduct.call]/4.3), each from the deductions made before A was
tried (clang's `DeducedOrig`): what the failed match of A deduced
(`I = ref` from `I<B>` against `ref<iface>`) is dropped (WP-789,
`tests/conformance/temp.deduct.call/base-after-template-template-mismatch-run.cpp`). An `A` that is a class template specialization
not yet instantiated is instantiated first, at the deduction's location
and without the undefined-template diagnostic, so its bases exist
([temp.inst]/2; clang's `isCompleteType` in `DeduceTemplateBases`; WP-682,
`tests/conformance/temp.deduct.call/derived-base-instantiates-argument-class.cpp`,
`libstdcxx-get-uninstantiated-tuple.cpp`: libstdc++ `std::get` on a
`tuple&` that a qualified call reaches with no ADL to complete it).
`DerivedClass` is set only when P is a template-id or a pointer to one.
Through a second pointer, `F<T>**` against `X<A, B>**` is a plain type
match, so a mismatch neither tries the bases nor instantiates `X<A, B>`
(clang's `isSimpleTemplateIdType` in `getTDF`; WP-787, Boost.Proto
`template_arity`,
`tests/conformance/temp.deduct.call/pointer-to-pointer-no-derived-class-deduction.cpp`).
The success must be unique; otherwise the
result is `MiscellaneousDeductionFailure` (clang). The template of `A` (or
of a base) is the class template it is a specialization of: alias
template sugar is looked through ([temp.alias]/2; clang's
`getLastTemplateSpecType`), so a base written `A<T>` with
`template<class T> using A = S<T>` deduces against `S<T>`
(`written_template_name`; WP-381,
`tests/conformance/temp.deduct.call/base-alias-template-sugar.cpp`;
libstdc++ `variant::operator=`/`emplace`).
A non-type argument of `P` that is not a deduced parameter but a value
already (a written constant, or an explicitly given parameter substituted
into `P`, [temp.deduct]/2) is compared with `A`'s value during the match:
unequal values are `NonDeducedMismatch`, so the base walk moves on to the
next base (`check_nondeduced_value`; clang converts the substituted
argument to an integral one). `std::get<1>(tuple)` matches
`_Tuple_impl<1, _Head, _Tail...>` against the second base, not the first
(`tests/conformance/temp.deduct.call/libstdcxx-base-explicit-nttp-mismatch.cpp`,
WP-332). A class-type value ([temp.param]/8) matches only a value of the
same class type naming the same template parameter object, so an
explicitly given class-type tag never matches a base whose argument is an
integer or another class value (`check_nondeduced_class_value`; WP-789,
`tests/conformance/temp.deduct.call/explicit-class-nttp-selects-base-run.cpp`).

A dependent class template-id `P` written with fewer arguments than its
template has is first completed with the template's default arguments
([temp.arg.general]/9), each substituted with the arguments before it, as
`A`'s were (`complete_dependent_args`; clang converts a dependent
template-id's arguments up front): `basic_string_view<_CharT>` is matched
as `basic_string_view<_CharT, char_traits<_CharT>>`. The parameters, their
defaults and their depth are the head declaration's (`head_template`, as
in `check_template_args`; the shared rule is §4.8.1), never the first
declaration's: that may be a
friend declaration inside a class template ([temp.friend]/1; libc++'s
`friend class basic_string_view` in `__wrap_iter`), whose parameters sit
one level deeper, so substituting at its depth would leave the default
unsubstituted and deduce the default's own parameter as the function
template's parameter of the same index (WP-286; also libc++ `vector<T>`
against `vector<int, allocator<int>>`, `__wrap_iter` befriending `vector`,
WP-295). A default that cannot be formed leaves `P` short and the
deduction fails as before. Tests:
`tests/conformance/temp.deduct.type/default-arg-friend-first-declaration*.cpp`,
`tests/conformance/temp.deduct.type/default-completion-friend-first.cpp`,
`tests/conformance/format.functions/libcxx-format-run.cpp`.

A template template parameter's template-id `U<Ps...>` against `A` with
another number of arguments (packs aside; `deduce_ttp_id_args`, WP-812)
deduces `Ps` from `A`'s first arguments, and `A`'s remaining arguments
must be the defaults those give ([temp.arg.template]/3, P0522, CWG2398 as
clang deduces it): `f(const U<T>)` with `template<class> class U` is
viable for `rv<double, 1>` of `template<class, size_t = 1> struct rv`
(Boost.Math's `sinhc_pi` for its reverse-mode `rvar`) and not for
`rv<double, 2>`. When `A` itself names a template template parameter (the
unique type of partial ordering, [temp.func.order]/3) the remaining
arguments are not checked (they bind as the deduced template's defaults,
P3310R5's `DeducedTemplateName`); a shorter such `A` matches under clang
19, which ignores `P`'s extra arguments, and fails at `-fclang-compat=22`,
so `f(TT<int, int>)` beats `f(TT<int>)` only there. The remaining
arguments are not checked in the partial ordering of function templates
either, where `A` is the other template's transformed parameter type:
`U<T>` is deduced from `num<B', E'>` but `num<B, E>` is not deduced from
`U'<T'>`, so `f(const num<B, E>&)` is more specialized than
`f(const U<T>)` (clang 19 and 22; Boost.Multiprecision's `sinhc_pi`
against Boost.Math's). The partial ordering of class template partial
specializations still checks them, comparing a defaulted non-type
argument by value: the partial specialization `lambda<protect<T>, Tag>`
keeps `protect<T>`'s default `0` as an expression, which equals the
completed default `0`, so it is more specialized than
`lambda<F<T1>, Tag>` (Boost.MPL). Tests:
`tests/conformance/temp.arg.template/p0522-deduce-defaulted-args-run.cpp`,
`tests/conformance/temp.arg.template/p0522-partial-order-defaulted-args-run.cpp`,
`tests/conformance/temp.arg.template/p0522-partial-spec-order-defaulted-value.cpp`,
`tests/conformance/temp.arg.template/p0522-partial-order-unique-ttp-clang{19,22}.cpp`,
`tests/conformance/temp.arg.template/p3310-probe.cpp`.

**Non-deduced contexts:**
- the nested-name-specifier of a qualified-id;
- `decltype(e)`;
- an NTTP inside a subexpression (`N+1`);
- a parameter with a default argument that the call does not use;
- an overload set that contains a template;
- a braced list (except as in §6.1);
- a pack expansion that is not last;
- a pack-index (P2662);
- `_Atomic(T)` (WP-154: clang 19 does not deduce through an `AtomicType`,
  so `template<class T> void f(_Atomic(T)*)` called with an
  `_Atomic(int)*` fails to deduce `T`).

**Non-type parameter types** ([temp.deduct.type]/20, C++17 and later).
A deduced non-type parameter whose declared type is dependent also
deduces that type from the type of its argument, with
`SkipNonDependent` (clang's `DeduceNonTypeTemplateArgument`; a reference
parameter matches the argument's type as that reference): `template<class
T, T *P> struct S<P>` deduces `T` from `S<&g>`, and the deducibility
check of a partial specialization finds `T` deducible (WP-789,
tests/conformance/temp.spec.partial.general/auto-primary-typed-nttp-partial-run.cpp).

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

**Where the placeholder sits** (clang `getContainedDeducedType`,
[dcl.spec.auto.general]/3). The placeholder of a declared type is found
by walking pointers, references and function return types
(`contained_placeholder`); a function type's own trailing return type
is part of that walk, so `auto (*p)(int) -> auto (*)(int) -> auto = g;`
deduces the innermost `auto`. Deduction matches a function type
against the initializer's exactly except for the return type: equal
parameter types (top-level cv dropped), equal variadic, cv and ref
qualifiers, and an equal exception specification (a `noexcept`
initializer also matches a potentially-throwing declared type), else
"variable 'p' with type '…' has incompatible initializer of type '…'".
The result keeps the declared sugar (`int (*(*)(int))(int)` prints as
`auto (*)(int) -> auto (*)(int) -> int`), and a constrained placeholder
there is checked at its type-constraint, both for a variable and for an
invented parameter (§2.6).
Tests: `dcl.spec.auto/nested-return-placeholder-run.cpp`,
`dcl.spec.auto/nested-return-placeholder-diags.cpp`.

**Positions that forbid a placeholder** ([dcl.spec.auto.general]/2-6,
[expr.prim.req.general]/4; clang `err_auto_not_allowed` from
`GetDeclSpecTypeForDeclarator`, `templ/ctad_site.rs`). One diagnostic,
`sema_auto_not_allowed`: "'auto' (or 'decltype(auto)') not allowed
<position>", with clang's position selector, at the decl-specifier's
placeholder. The placeholder checked is the decl-specifier's own, or, when
the declarator's function declarator nearest the name has a trailing return
type, the one contained in that trailing type (`declarator_placeholder`;
`using F = auto (*)(int) -> int;` is fine, `-> auto` is not). GNU
`__auto_type` is not checked here. The position comes from the
declarator (`auto_declarator_site`):

| Declarator | Position |
|---|---|
| any `typedef` declarator | "in typedef" |
| alias-declaration type-id (`DeclSite::Alias`) | "in type alias" |
| template argument, type template parameter default (`TemplateArg`, `TypeParamDefault`) | "in template argument" |
| requires-expression parameter (`RequiresParam`) | "in requires expression parameter" |
| other type-ids (`sizeof`, `alignof`, casts, `typeid`, `__typeof__`, default-argument operands; `TypeName`/`Plain` without a name) | "here" |
| non-static data member (struct / union / class) | "in non-static {struct,union,class} member" |
| handler's exception-declaration | "in exception declaration" |
| parameter (§2.6 placement) | "in function prototype", "in lambda parameter", "in function prototype that is not a function declaration" |

An enum-base names a type-specifier without a declarator
(`type_specifier_placeholder_check`, "here"). The declarator recovers as
the error type (the parameter rule of §2.6 recovers as `int`), so its uses
do not cascade, and `typeid` of an error type stops before its
`<typeinfo>` check (clang's `ParseCXXTypeid`). A deduced class template
in an alias or type template parameter default gives clang's "requires
template arguments" (its `TemplateTypeArg` context), not "argument
deduction not allowed". Positions that deduce or invent are unaffected:
variables, `new`, NTTPs, return types, structured bindings (a `typedef` one
gets only the decomposition error, `dcl.struct.bind/specifiers.cpp`), conditions,
range-for, conversion functions, generic lambdas and abbreviated templates.
Tests: `dcl.spec.auto/p5-auto-not-allowed.cpp` (every position above),
`dcl.spec.auto/p5-auto-allowed.cpp`,
`expr.prim.req.general/p4-auto-param.cpp`.

**Uses:**
- variables;
- `new auto(…)`;
- NTTPs (`template<auto V>`, `C auto V`, `decltype(auto) V`): the type is
  deduced from the argument;
- deduced return types: each later `return` must deduce the same type,
  else "'auto' in return type deduced as 'X' here but deduced as 'Y' in
  earlier return statement";
- generic lambdas (WP-075).

In a function template (or a member of a class template) the pattern
does not deduce: its return type is a dependent `auto`, and each
specialization deduces from its own instantiated, non-discarded returns
(sema.md §4.3.1, WP-122).

A function with a deduced return type that is used before its type is
known has its body **instantiated immediately**. That is not the
immediate context: errors are hard.

### 6.4 Explicit args and default arguments

- **Explicit prefix of the trailing function parameter pack.** Only the
  first template parameter pack takes explicit arguments
  ([temp.arg.explicit]/9). Its explicit length is snapshotted before
  deduction. It is the trailing function parameter pack's prefix only
  when that pack's pattern (`U`, `U&&`, `U*`) expands this first pack.
  Otherwise the prefix is empty, so a pack deduced earlier from
  `tys<Tf...>` does not shorten `Up...`. When the pattern also names
  other packs (`pair<T, U>...` with `T` explicit), those are deduced
  from every element and the explicit pack is not extended: it keeps
  its explicit elements, and a mismatch in an explicit element fails
  deduction (clang 19, observed). If the pattern names only the
  explicit pack, elements before its explicit length are not deduced.
- **Non-trailing function parameter packs** ([temp.deduct.type]/5.7)
  are a non-deduced context and take no call argument. After the
  explicit arguments are substituted, one whose packs are all given
  is expanded to its explicit elements; it still counts as one
  parameter in the too-many-arguments check (clang retains the
  expansion). A pack that is left unexpanded (not all given) skips the
  too-many-arguments check before deduction (clang `isTemplateVariadic`:
  a pack anywhere, not only trailing; WP-311). An undeduced pack is then
  empty, and the arity is checked on the specialization (`candidate
  function [with T = <>] not viable: requires 1 argument, …`; test
  `temp.deduct.call/non-trailing-pack-arity.cpp`). A parameter pack never counts
  towards the minimum argument count (`fn_min_args`, clang
  `getMinRequiredArguments`), wherever it is.
- **The explicit-specifier** ([temp.deduct.general]/7, [dcl.fct.spec]/3).
  A value-dependent `explicit(expr)` (kept in `decls.cold.explicit_expr`)
  is substituted once the specialization's signature is formed and, in
  call deduction, only after its associated constraints are satisfied
  ([temp.deduct.general]/5; clang `instantiateExplicitSpecifierDeferred`):
  `get_function_spec_signature` leaves it pending
  (`templ.explicit_pending`) and `finish_function_spec_explicit`
  substitutes it, so a candidate whose constraint fails never forms an
  `explicit(expr)` ill-formed for its arguments (libstdc++ `tuple`'s
  `explicit(!__convertible_from_tuple_like<_UTuple>())`; test
  `temp.deduct.general/constraints-before-explicit-specifier.cpp`).
  `get_function_spec` does both. `explicit(false)` makes the
  specialization non-explicit, and a substitution failure is a deduction
  failure. A member of a class
  template substitutes it when the class is instantiated. A template is
  "not a candidate" before deduction only when it is explicit
  non-dependently (clang `isNonDependentlyExplicit`;
  `Sema::nondependently_explicit`). That holds for a constructor template
  in a user-defined conversion's copy-initialization too
  ([over.match.copy]/1.1; `Sema::drop_explicit_ctor_templates`): it is
  noted "explicit constructor is not a candidate" and never deduced, so a
  substitution that would be ill-formed outside the immediate context
  (libc++'s explicit `basic_string(const T&)` with `T = void`) is not
  reached (WP-694; test
  `over.match.copy/explicit-ctor-template-not-deduced.cpp`).
- **Return types** ([temp.deduct.general]/11.10). A substituted
  signature (or any substituted function type) whose return type is an
  array or function type fails with `function cannot return
  {array|function} type T`, a deduction failure under SFINAE (clang
  `BuildFunctionType`; `Subst::check_return_type`, WP-201). It is
  reported where the function type begins (clang's `FunctionTypeLoc`
  begin): a member function's return type as written (WP-222), and for
  a function type inside a parameter's, variable's, data member's,
  typedef's or alias's declarator (`T (*p)()`, `static const T (*s)()`,
  `using U = T (*)();`) the declaration's first type-specifier, recorded
  for dependent declarations in `Sema::type_spec_locs` and carried as
  `Subst::type_begin` while the declaration's type is substituted
  (cleared inside the function type's parameters; WP-241). Without a
  recorded location the declarator's name is used.
- **A reference to `void`** formed by substitution ("cannot form a
  reference to 'void'", [dcl.ref]/1) is reported at the `&`/`&&` that
  forms it (clang's `ReferenceTypeLoc`): the outermost reference chunk of
  a dependent field's, variable's, static data member's, typedef's or
  parameter's declarator is recorded in `Sema::ref_locs` (a parameter
  carries it as `ParamInfo::ref_loc`; a parameter pack does not) and
  carried as `Subst::ref_loc`, which the first reference rebuilt takes
  (WP-300). An alias-declaration records none and is reported at its
  name, as clang does.
- **A pointer to a reference or an array of references** formed by
  substitution ([dcl.ref]/5, [dcl.array]/1; also every other check of
  the pointer or array rebuilt) is reported at the innermost written `*`
  or `[` of the declarator, as clang's `TreeTransform` rebuilds the type
  at its `TypeLoc` sigil: the first `*` of `T const *volatile *m`, the
  return type's `*` of `T *h()` and `T *(*fp)(int)`. That chunk, when it
  applies to the decl-specifiers, is recorded for a dependent field,
  variable, static data member, typedef, alias-declaration, function
  (its return type), parameter (`ParamInfo::chunk_loc`) and type
  parameter default in `Sema::inner_chunk_locs`, and carried as
  `Subst::chunk_loc`, which the first pointer or array rebuilt takes.
  The diagnostic names the declaration (`Subst::declarator_name`: the
  member, function, typedef, alias or parameter, and for a substituted
  default the type parameter, `'U' declared as a pointer to a
  reference`); a parameter of a function type is a `'type name'`.
  Without a recorded chunk the declarator's name location is used
  (WP-437). A type parameter default whose type-id is invalid in the
  definition (`class U = R<T> *`, `R<T>` a reference) is dropped, as
  clang's `ActOnTypeParameter` drops an invalid `TypeResult`.
- **Default template arguments of function templates** are used only for
  parameters that are neither deduced nor given explicitly. They are
  substituted in order and may name earlier parameters.
- **Default function arguments** are instantiated at the call that uses
  them, under the frame `DefaultFunctionArgumentInstantiation` (a hard
  error). The note is "in instantiation of default function argument
  expression for 'f<int>' required here". The instantiated argument is
  cached per instantiated `ParmVar` in `default_args`.
- A braced default argument (`C<sizeof(T)> c = {}`, `int y = {sizeof(T)}`)
  is stored in the pattern as its `InitListExpr`. When it needs a rebuild,
  substitution rebuilds the list's elements and hands them to
  `act_on_param_default_list`, which list-initializes the instantiated
  parameter's type (the same path as a non-template braced default
  argument; WP-136).
- A default argument of a parameter whose type is, or is a reference to,
  a dependent class ([temp.dep.type]/7: a nested class of a class
  template) is stored as written, unconverted (a braced one as the
  `void` list of `dependent_init_list`), and converted only when it is
  instantiated, where the class is complete (WP-690). The template
  definition never looks at the class's constructors or default member
  initializers for it, so `const H &h = {}` with an `H` that has default
  member initializers is valid there.

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
- **In a discarded statement** of a non-template function ([stmt.if]/2;
  clang's `OdrUseContext::FormallyUsed`, `Sema::in_discarded_statement`)
  nothing is odr-used: only `constexpr`/`consteval` functions (including
  implicit special members that are constexpr) are defined or
  instantiated, for constant evaluation; nothing is queued or marked
  used, and a variable not usable in constant expressions is not
  instantiated. A deduced return type is still deduced at the reference.
  Nested contexts (constant-expressions, `if consteval`, nested
  `if constexpr`) stay discarded; a lambda body does not. At compat 19 a
  written variable initializer is not discarded (clang 19/20). Tests:
  `stmt.if/discarded-statement-*.cpp`.
- **`constexpr`, `consteval` or deduced-return specializations** are
  instantiated **immediately**, under the frame `TemplateInstantiation`
  with poi `loc`. This keeps clang's diagnostic order and makes the body
  ready for the Oven (Q7). A specialization whose definition is already
  being instantiated further up the instantiation stack (mutually
  recursive constexpr templates, one forward-declared, as libc++'s
  `__stable_sort` and `__stable_sort_move`) is not instantiated again:
  its definition is in progress, as clang's `isDefined` is once
  `InstantiateFunctionDefinition` has started. Test:
  `temp.inst/constexpr-mutual-recursion.cpp`. The same holds for a
  member function of a class template specialization (a move
  constructor calling a constexpr `swap` that moves `*this`). Test:
  `temp.inst/constexpr-members-mutually-recursive-run.cpp`.
- **Every other specialization** is pushed onto `pending`. It is
  instantiated at end of TU, or earlier through `ensure_body`.
- **Explicit-instantiation-declaration members** are queued only if they
  are inline or `constexpr`.

### 6.6 CTAD in M3 ([over.match.class.deduct])

On the first use of CTAD for a class template `X`, the guide set is
built and cached in `guides[X]` (from the definition's template, never a
friend first declaration's deeper head, §4.8.1):
- one implicit guide per constructor of the primary pattern;
- the copy-deduction candidate `X(X<T…>) -> X<T…>`;
- every namespace-scope `CXXDeductionGuide` declared so far;
- the guides from inherited constructors (§6.6.1), formed on each use
  (the base may gain user guides later) and cached per source guide.
Building it uses the `BuildingDeductionGuides` frame.

The set then goes through overload resolution with the initializer as
the arguments. The result type is the guide's return type, after
substitution (for a guide from an inherited constructor, already the
derived class, or mapped to it at selection for a "late" guide, §6.6.1).
If several best guides tie, the
[over.match.best.general]/2.10–13 tie-breakers apply.

For list-initialization, overload resolution is [over.match.list]:
initializer-list guides first, with the whole list as the argument.
Without user guides, an aggregate class template adds the aggregate
deduction candidate (P1816). An alias template deduces through the
transformed guides of the template its defining-type-id names (P1814).
When a non-list argument has already been diagnosed (a `RecoveryExpr` or
an error type, such as a variable whose own deduction failed), nothing is
deduced and no further error is emitted. clang likewise deduces nothing
from an initializer that contains errors (WP-192).
An implicit guide's parameter types may name member typedefs of the class
template (libc++ `vector(initializer_list<value_type>)`). Substituting
the guide's arguments into such a typedef substitutes its underlying type
and keeps the typedef as sugar, without instantiating the class
(`Substituter::class_not_instantiated`). clang's guide transform clones
the typedef instead.
Any other member of the pattern that a guide's parameter types name (a
static data member or enumerator, `span<O, rank_>` in libc++'s
`layout_stride::mapping`, or a member of a member class) is found in the
specialization the substituted injected-class-name `X<P...>` names: that
class is completed and the member instantiated from the pattern member
is used ([temp.inst]/3; `Substituter::pattern_specialization`, clang's
`FindInstantiatedDecl` on a deduction guide; WP-302).

The guide of a constructor template may also name other members of the
class template in its signature or template parameters: a member class,
a member alias template, a static member function or member template of
a member class (libc++ `pair`: `__enable_if_t<_CheckArgs::template
__is_pair_constructible<_U1, _U2>(), int> = 0`). When the guide is
substituted with the deduced arguments, such a member resolves in the
specialization the substituted `X<P...>` names, made complete
(`Substituter::pattern_specialization`, from `find_instantiated_decl`;
clang's `FindInstantiatedDecl`). When an alias guide is built
([over.match.class.deduct]/3), the substituted class is still dependent
(`P<T, short>`): the member becomes a member of an unknown
specialization ([temp.dep.type]/6) — a `DependentIdentifier` template
name, a `DependentName` type or a `DependentScopeDeclRefExpr`, with the
qualifier `P<T, short>::` (`Substituter::dependent_member_nns`,
`in_dependent_specialization`) — and is looked up again when the alias
guide is used. A canonical pack-expansion argument (`Idx<I...>` in a
guide's canonical signature) expands like the written one
(`expand_arg`). The aggregate deduction candidate goes through the same
alias transformation. Guide signatures print such members qualified
(`typename P<T, short>::Chk::template same<U>()`) where clang prints
them as written (`Chk::template same<U>()`), and a member alias
template-id in a qualifier gains a stray keyword (`template
ChkDep<D>::ok()` for clang's `ChkDep<D>::ok()`): note-only differences
kept for a follow-up (the rebuilt dependent name has no written-form
sugar).
`tests/conformance/over.match.class.deduct/alias-ctor-template-members.cpp`,
`.../alias-aggregate.cpp`.

A template template parameter named alone (`C(args)`, `C{args}`,
`C x(args);`) is a placeholder for a deduced class type as in clang
(`is_ctad_template`, `names_class_type`): a dependent
`DeducedTemplateSpecialization` whose template name substitution replaces
with the argument's template, after which the placeholder is deduced as
above (libc++ `ranges::to`; WP-198).

**Landed (WP-071b, WP-134):** see spec/sema.md §4.3.2 for what is
implemented and what is deferred.

#### 6.6.1 Guides from inherited constructors (P2582, `ctad/inherit.rs`)

[over.match.class.deduct]/1, last paragraph (C++23): when class template
`C` is defined and inherits constructors from a direct base denoted in its
base-specifier-list by `B`, let `A` be the alias template
`template<C's parameters> using A = B;`. If `A` is deducible, the guides
of `A` are guides of `C`, each with its return type `R` replaced by
`CC<R>::type`: the specialization of `C` whose arguments are deduced from
`R` against `B`.
- **Which bases** (`inherited_ctor_bases`): each `Using` or (dependent)
  `UnresolvedUsingValue` member of `C`'s pattern definition named by a
  constructor name, whose nested-name-specifier is either a type equal
  (canonically) to a non-pack base-specifier's type, or `C::N` (`C` named
  by its injected-class-name, `N` a member of the unknown specialization,
  [temp.dep.type]/6) where `N` is the name of the class template of exactly
  one base-specifier (`dependent_base_named`; `using G::F::F;` in the
  standard's example). Only a `B` that is a specialization of a class
  template contributes. The definition's `HAS_INHERITED_CTOR` bit
  short-circuits the scan.
- **Deducible** `A`: every template parameter of `C` is deduced from `B`
  against itself (`alias_deducible(C, B, B)`; an undeduced pack is
  empty). `template<class T> struct E : B<int> { using B<int>::B; };`
  gets no guides from `B`.
- **Forming** (`inherited_guide`, cached in `ctad_alias_cache` under
  `(C, g)`): the guides of `B`'s template are its full set (implicit,
  copy, user-declared, and the ones `B` itself inherited, so inheritance is
  transitive; no aggregate candidate). A guide template goes through the
  alias transform (`build_alias_guide(C, B, g, inherited = true)`, §6.6)
  and its substituted return type `R` is deduced against `B` right away,
  symbolically on the new guide's own parameters
  (`inherited_guide_result`; a pack expansion in `B` deduces the whole
  pack): the guide's return type becomes `CC<R>::type`, written with the
  new guide's parameters, and the `__is_deducible` constraint is not
  attached (a symbolic match implies it). Each formed guide is recorded in
  `templ.ctad_inherited` as `InheritedGuide { class, base, source_ret }`,
  `source_ret = Some(R)`. When the symbolic deduction fails the guide
  keeps `R` as its return type and the alias constraint
  (`templ.ctad_alias`), `source_ret = None` (a "late" guide), and the
  result is mapped at selection. A non-template guide is cloned with
  `C<...>` as its return type, or dropped when `CC<R>` does not match.
- **Alias templates and further inheritance** use the formed guides like
  any other guide of `C` (`ctad_guides`, §6.6): an alias template of `C`
  and a class that inherits from `C` both see them, and the alias guide of
  an inherited guide is itself recorded as inherited (keeping /2.10). Late
  guides are not passed on.
- **Result** (`ctad_result_type`): the selected guide's return type is the
  result, except for a late guide, whose substituted `R` is deduced against
  `B` and `C`'s injected-class-name substituted with the deduced
  arguments.
- **Ranking** ([over.match.best.general]/2.10, `better`): a guide not
  formed from an inherited constructor beats one that is when their
  parameters for the arguments have the same types (`same_guide_params`).
  It comes before /2.11 (user-declared over implicit).
- **Notes** print such a guide as an alias guide is printed, with the
  constraint naming `R` and the return type naming `C`:
  `requires __is_deducible(C, B<X *>) C(X *) -> C<X>` (no clang text
  exists).
- `__cpp_deduction_guides` stays at the value clang defines (201703L in
  clang 19 and 22): kiln adopts 202207L only once the oracle clang
  implements P2582, so the predefined macros keep matching the oracle.

Tests: `tests/conformance/over.match.class.deduct/p2582-inherited-guides.cpp`
(the standard's example with `F<U, T, int>` written out, constructor
templates, user guide templates, /2.10, packs, casts and `new`),
`.../p2582-not-deducible.cpp` (`D`, `E` and a non-matching `R`),
`.../p2582-transitive-alias.cpp` (transitive guides, packs, alias
templates of inheriting classes, `using G::F::F;`),
`tests/conformance/temp.dep.type/p6-inherited-ctor-injected-base.cpp`,
`.../run-p2582.cpp`, `.../p2582-probe.cpp`.

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
   pair. A constant `P` argument (`f(ie<I, 1>)`) does not deduce from a
   value-dependent `A` argument (the other side's `N`): it is a
   mismatch, as clang's integral `P` against an expression `A`
   (`check_nondeduced_value`; WP-669,
   `tests/conformance/temp.func.order/p3-constant-argument-more-specialized.cpp`).
   Packs follow /8–11. A template argument pack expansion in the `A`
   types (`L<I...>`, of the argument template's own pack) is deduced as a
   whole by a `P` pack expansion (`DeducedArgs::expansions`, as for
   partial specializations), so the two packs' template-parameter
   indices need not coincide (WP-726,
   `tests/conformance/temp.func.order/p3-pack-expansion-other-index.cpp`).
   A template parameter of the deduced template left without a value
   fails the deduction when the compared parameter types name it
   anywhere ([temp.deduct.partial]/12; clang `isAtLeastAsSpecializedAs`
   with `MarkUsedTemplateParameters` without `OnlyDeduced`): `order.rs`
   `undeduced_params_unused` walks the canonical compared types
   (`mark_used_type`), through nested-name-specifiers, template
   arguments, template names and expressions (`sizeof(T)`, `N + 1`,
   `X<T>::value`), marking type, non-type and template template
   parameters and packs of the deduced template's depth alike. Names
   gone from the canonical type (an alias template that drops its
   parameter, a non-dependent `decltype`) and the call's return type do
   not count. So `to_pointer(buffer_appender<T>, size_t)` beats
   `to_pointer(OutputIt, size_t)` for `to_pointer<char>(it, n)` (WP-726),
   and the same holds for a non-type, template template or pack parameter
   (WP-779, `tests/conformance/temp.func.order/p12-undeduced-used-*.cpp`,
   clang 19 and 22 agree).
   A pack named by the pattern of a `P` pack expansion that matches no
   `A` element has a value: the empty pack ([temp.deduct.type]/9; clang
   `PackDeductionScope::finish`; `DeducedArgs::empty_expanded`). So
   `f(L<>, F&&)` is more specialized than `f(L<T...>, F&&)` (WP-787,
   `tests/conformance/temp.func.order/p12-empty-expansion-deduces-empty-pack-run.cpp`).
   The last tie-breaker for packs is
   [temp.func.order]/5: a function without a trailing pack wins.
5. **Constraints.** If both directions succeed, the more constrained one
   wins ([temp.func.order]/6, P2113; subsumption §8.4), but only when the
   constraints are comparable (`constraints_comparable`, clang
   `getMoreSpecializedTemplate`; WP-339): the template-parameter-lists
   and the function parameter lists (all declared parameters, after the
   /3 object parameter and reversal) have the same length (/6.1), the
   corresponding template parameters are equivalent ignoring their
   type-constraints (same kind, packness, non-type parameter type
   ignoring a placeholder's constraint, template template parameter
   head) and the positionally corresponding
   parameter types are the same ignoring top-level cv (/6.2.2), and, for
   a conversion, the conversion types are the same. Otherwise neither is
   more specialized: for CTAD the later [over.match.best]/2.11-2.13
   tie-breakers decide (a constrained constructor guide
   `zt(const tp<D>&) requires C` against the user guide `zt(tp<D>)` of a
   two-parameter class template: the user guide wins).
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

**Template template parameter kinds** (P2841R7, [temp.over.link]/6.1). A
type, a concept and a variable template template parameter are three
kinds: template-heads whose corresponding template template parameters
differ in kind are not equivalent (`heads_equivalent`,
`template_params_equivalent`, and the redeclaration check
`params_match`, which reports "template parameter has a different kind in
template {|template parameter }redeclaration" as for a type/non-type
change, nested template template parameters included). So
`template <template <class> class K> int h()` and the same with
`concept K` or `auto K` are three overloads, each removed by an argument
of another kind, while renaming a concept or variable template template
parameter redeclares. A requires-clause `C<T>` naming a concept template
parameter is compared like any expression naming template parameters
(by position), so `requires C<T>` and `requires D<U>` with `C`/`D`
corresponding are equivalent and the second declaration redeclares the
first. *Wording over clang:* clang 22 does not compare template template
parameter kinds across class template redeclarations and keeps the two
`requires C<T>` / `requires D<U>` declarations as distinct candidates
(an ambiguous call); kiln follows [temp.over.link]
(`temp.over.link/concept-variable-ttp-kind-redeclaration.cpp`,
`temp.over.link/concept-ttp-requires-redeclaration.cpp`; the agreeing
cases are `temp.over.link/concept-variable-ttp-redeclaration.cpp`).

Constructors, conversion functions and the other special members follow
the same rule ([temp.over.link]/7). A member template is a redeclaration
only when both its template-head and its function type are equivalent.
Two constructor templates that differ only in an `enable_if` NTTP are
therefore distinct entities.

**Implicit `this` in class templates** ([temp.dep.expr]/3). An implicit
member call `f()` to an overloaded member of a class template is
resolved at instantiation, as `this->f()` would be. This also holds for
qualified `S::f()`, whose injected-class-name maps to the
instantiation.

### 6.9 Explicit specialization and instantiation of function templates

For `template<> R f<args>(params)` or `template<> R f(params)`, and for
`template R f(params);`:
1. Collect the function templates named `f` in the scope.
2. Deduce each one from the declared function type ([temp.deduct.decl];
   `deduce_decl`). Neither top-level exception specification takes part.
   Nothing is deduced from it, so `template void f() noexcept;` of
   `template <bool B> void f() noexcept(B)` is "couldn't infer template
   argument 'B'". The template's specification is not substituted with
   the explicit arguments, so `noexcept(T::value)` is no substitution
   failure. This is clang's `adjustCCAndNoReturn(..., AdjustExceptionSpec)`.
   A mismatch elsewhere in the type prints both whole types with their
   specifications ("could not match 'void (int) noexcept' against 'void
   (long)'"). Then the specialization's specification is instantiated
   if it is still pending ([temp.inst]/14) and compared (WP-322):
   - **Explicit instantiation** ([temp.explicit]/9, [except.spec]/5;
     `check_inst_exc_spec`, clang `ActOnExplicitInstantiation` →
     `CheckEquivalentExceptionSpec`). One that writes no specification
     takes the template's. This covers libstdc++'s `extern template bool
     has_facet<...>(const locale&)` of a `throw()` template
     (`tests/conformance/temp.explicit/libstdcxx-explicit-inst-exception-spec.cpp`).
     One that writes a specification must agree with the instantiated
     one: both non-throwing, or both potentially-throwing. Here no
     specification does not match `noexcept(false)`: clang does not set
     `AllowNoexceptAllMatchWithNoSpec`, and a failed instantiation counts
     as none. Otherwise the error is "exception specification in
     explicit instantiation does not match instantiated one", at the
     decl-specifiers. Its note "explicit instantiation refers here" is
     at the specialization's latest declaration: the template, or an
     earlier explicit specialization. The explicit instantiation then
     has no effect, and this is checked before §4.5's kind checks.
   - **Explicit specialization, or a friend naming a specialization**
     ([temp.expl.spec], [except.spec]/5; `check_spec_exc_spec`, clang
     `MergeFunctionDecl`). It redeclares the specialization, whose
     specification is the instantiated one, moved to the new name. One
     that writes none where the instantiated one is non-throwing gets
     "'h<int>' is missing exception specification 'noexcept'". Any other
     mismatch gets "exception specification in declaration does not
     match previous declaration". Each has "previous declaration is
     here" at the new name, and the declaration is invalid. Here no
     specification matches `noexcept(false)`. The check comes after the
     after-instantiation check (§4.5).
   Tests: `tests/conformance/temp.explicit/exception-spec-match.cpp`,
   `tests/conformance/temp.expl.spec/exception-spec-redeclaration.cpp`.
   Left: a friend whose noexcept-specifier waits for the end of its
   class is not compared, where clang checks it later. Instantiating an
   explicit instantiation definition does not instantiate a pending
   specification, which clang does in `InstantiateFunctionDefinition`.
3. Take the most specialized of those that succeed (§6.7).
4. If none or several remain, the errors are clang's: "no function
   template matches function template specialization 'f'" and
   "function template specialization 'f' ambiguously refers to more than
   one function template".
5. When nothing named `f` is visible (an unqualified declarator-id
   without template arguments), clang declares a plain, invalid function
   after the error, and so does kiln (`act_on_function_spec_declarator`
   returns `Err(())`, WP-318): a later call resolves to it and reports
   nothing more
   (`tests/conformance/temp.expl.spec/no-matching-template-declares-function.cpp`).
6. A class-scope explicit specialization in a dependent class (an
   unqualified member declarator inside a template parameter scope) whose
   lookup finds no member function template of the name reports clang's
   `err_dependent_function_template_spec_no_match`, "no candidate
   function template was found for dependent member function template
   specialization", at the declarator-id, with "candidate ignored: not a
   function template" at each declaration found; the declaration is
   dropped (`sema_dependent_member_spec_no_candidate`, WP-330;
   `tests/conformance/temp.expl.spec/dependent-member-spec-no-candidate.cpp`).
   The friend form is the `FriendSpec` entry of §11.2. When candidates exist,
   kiln still matches in the pattern, where clang defers the match to
   the class's instantiation (kept deviation: a mismatch is reported at
   the definition, without the "in instantiation of" note).

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

**As built (WP-106, `templ/nttp.rs`).**
- **Type check** (`check_nttp_type`, clang `CheckNonTypeTemplateParameterType`
  / `RequireStructuralType`), at the parameter declaration
  (`act_on_non_type_template_param`; the parameter is marked `INVALID`)
  and again when a class argument is converted:
  - an rvalue-reference type: `sema_nttp_rvalue_ref`;
  - an incomplete class is accepted (checked at use);
  - not literal: `sema_nttp_non_literal` plus one `note_non_literal_*`
    (no constexpr constructor and not an aggregate; a non-literal base;
    a non-literal data member; a non-constexpr destructor), clang's order;
  - not structural: `sema_nttp_not_structural` plus the first reason:
    a non-public or mutable member, an rvalue-reference member, a
    non-public base, then the first member/base of non-structural type
    (`note_not_structural_subobject`), recursing into it. Structurality is
    recomputed (`is_structural_at`), because `DefinitionBits::STRUCTURAL`
    misses rvalue-reference members (class owner).
  - Dependent types are checked when substituted (not yet: §11.2 gaps).
- **Class arguments** (`convert_class_nttp_arg`, hooked from
  `args.rs::convert_nttp_arg`): copy-initialize a `const T` from the
  argument, `eval_const` with `EvalContext::converted`, then
  `AstBuilder::template_param_object(const T, v)`; the argument is
  `TemplateArg::value(canonical T, v)`. Aggregate values are interned, so
  the `ConstValueId` is the (type, value) key.
- **`_Complex` arguments** (WP-810): a GNU complex type is structural (a
  scalar, as clang). The argument is `TemplateArg::value(T, v)` with `v`
  the frozen `T[2]` image; substituted, it is a `ConstantExpr` holding `v`
  around an opaque prvalue (`nttp_value_expr`), whose parts the Oven reads
  from the image (`cx_const`) and codegen emits as a constant. Two
  arguments are the same when their images are identical (`-0.0` differs
  from `+0.0`). It mangles as clang: `X tl <type> <re> <im> E E`, trailing
  `+0` parts dropped (`AstNames::shape_at` gives the complex type the
  `Array` shape of `T[2]`).
- **The object** is a `TemplateParamObject` decl (`IMPLICIT`, unnamed, in
  the TU, in no lookup or member list) memoized in
  `TemplateTables::{param_objects, param_object_values}`
  (`param_object(v)`, `param_object_value(d)`).
- **Class template placeholders** (`template<std::array A>`,
  [temp.param]/6.3): `convert_nttp_arg` deduces the type per argument by
  CTAD as for `C x = e;` (`ctad_for_initializer`, copy-initialization),
  then converts as a class argument. A use of such a parameter in the
  pattern is type-dependent (`QualType::DEPENDENT`). After substitution,
  `decltype(A)` of a template parameter object is the deduced class type
  without `const` ([dcl.type.decltype]/1.3; `decltype_of` on a
  `SubstNonTypeTemplateParmExpr` of class type); `decltype((A))` is
  `const T&` (WP-201).
- **Uses.** An id-expression naming a class-type NTTP in the pattern is an
  lvalue of type `const T` (builder `decl_ref_category`,
  `build_decl_ref_extra`); a reference NTTP names its referent (an
  lvalue). Substitution (`subst/exprs.rs::value_literal` →
  `nttp_value_expr`) builds the replacement: a `DeclRefExpr` to the
  object (class), the entity (reference), `&entity` / the decayed array
  (pointer; an array is not decayed for a parameter of pointer-to-array
  type, where `&arr` is the argument: `template<auto D>` with
  `D = &C::arr`, WP-690,
  `tests/conformance/temp.arg.nontype/pointer-to-array-argument.cpp`),
  `nullptr` converted to the parameter type, or, for a
  non-null pointer to member (WP-415), `&C::m` of the member's declaring
  class `C` (an `AddrOf` of a `DeclRefExpr` to the member, as
  `member_pointer_of` builds it) converted to the parameter type along
  the value's path: implicitly towards a derived class, by a
  `static_cast` when the value points to a derived class's member
  (`Scalar::MemberPtr::derived`), as clang
  `BuildExpressionFromDeclTemplateArgument`. Argument identity is the
  value's (`member`, `derived`, `path`), so `X<pb>` and
  `X<static_cast<int D::*>(&B::b)>` are one specialization; the mangled
  names match clang's (`XadL_ZN1S1vEE`, `mcM1BiadL_ZN1D1dEE`;
  `tests/codegen/nttp_member_pointer.cpp`,
  `tests/conformance/temp.arg.nontype/member-pointer-argument-value.cpp`).
  `&C::m` of exactly the parameter's type while `C` is still being
  defined (boost.intrusive's `member_hook<C, H, &C::hook>` in `C`'s own
  body) is converted without the Oven, which has no layout for `C` yet:
  the value is the member itself, the same argument as after `C` is
  complete (`args.rs` `defining_class_member_ptr`; WP-726,
  `tests/conformance/temp.arg.nontype/member-pointer-of-class-being-defined.cpp`).
- **Reference arguments** (`convert_ref_nttp_arg`): the conversion must
  exist (`sema_cce_not_implicitly_convertible`), must not bind a
  temporary (`sema_cce_ref_binds_temporary`), and the referent must not
  be automatic ("reference to 'x' is not a constant expression"). The
  value is the built-in `&` of the bound glvalue, re-interned at the
  reference type so it prints as `T<gi>`.
- **Disallowed bases** ([temp.arg.nontype]/6, `args.rs`
  `invalid_template_arg_base`): a pointer argument, or the address a
  reference argument binds, whose base is a string literal, a `typeid`
  result or a temporary is an error, "{pointer|reference} to
  [subobject of ]{string literal|type_info object|temporary object} is
  not allowed in a template argument" (clang
  `err_constexpr_invalid_template_arg`); "subobject of" when the value
  is not the whole object (a decayed literal, a non-zero offset). For an
  explicitly-specified argument of a function template the error is
  deduction's invalid-argument failure, so `make<"ab">` drops a
  `template <auto p>` candidate and picks one whose parameter is a class
  type (WP-792; `temp.arg.nontype/string-literal-typeid-argument.cpp`,
  `string-literal-argument-sfinae-run.cpp`). Gap: clang words a
  `__func__` argument "predefined '__func__' variable"; kiln says
  "string literal".
- **Oven.** `var_info` treats the object as a usable constant whose value
  is its `ConstValueId`; `lv()` addresses it as a global.
- **Codegen.** `param_object_global`: a `linkonce_odr dso_local constant`
  in its own comdat (clang `GetAddrOfTemplateParamObject`).
- **Mangling seam** (`kiln_abi::mangle::{aggregate_arg, mangle_param_object}`):
  `_ZTA` + `Xtl <type> (Ly<word>E)* EE`, the value's 8-byte little-endian
  words with trailing zero words trimmed. Deterministic and distinct for
  distinct images, but **not clang's spelling** (clang walks the members:
  `Xtl1PLi1ELi2EE`; that needs the record layout in the mangler, and
  pointer members are not distinguished). Owner: WP-079. Reference and
  pointer arguments mangle as clang (`L_Z2giE`, `XadL_Z1gEE`,
  `Xadso<elem>L_Z3arrEEE` for a decayed array).

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
| structured binding pack (pattern, P1061) | a `Binding` in `Sema::binding_packs`, of type `PackExpansion(DEPENDENT)` |
| structured binding pack (decomposed) | N `IMPLICIT` `Binding`s of the pack's name (`Sema::binding_pack_elems`) that replace it in the decomposition's binding list (`Decls::expand_decomp_bindings`) |
| friend pack (P2893) | a `Friend` whose `FriendTarget::Type` is `PackExpansion(T)` |

**Structured binding packs** ([dcl.struct.bind]/1, [temp.variadic]/5,
P1061, WP-359). `auto [a, ...xs] = e;` is allowed only in a templated
entity (`sema_pack_outside_template`, clang 22 "pack declaration outside
of template"), with at most one pack (`sema_decomp_decl_multiple_packs`
with `note_previous_binding_pack`); either error drops the declaration,
as clang does. Before C++26 it warns `parse_ext_binding_pack`
(`-Wc++26-extensions`). In the pattern the decomposition is never
decomposed, even for a non-dependent initializer (clang 22): every
binding is dependent and the pack an unexpanded `PackExpansion(DEPENDENT)`.
`is_pack_decl` counts the pack, so `sizeof...(xs)`, `xs...`, folds and
`xs...[i]` see it. When the instantiated decomposition knows its element
count `n` (array, tuple-like, member-wise), `expand_binding_pack`
introduces `n - (bindings - 1)` implicit bindings of the pack's name and
the count check treats the pack as that many names
(`wrong_number_of_bindings` when there are fewer elements than non-pack
names; its text is clang 19's "decomposes into N elements", where clang 22
says "binds to"/"cannot bind" and quotes the pack in "invalid index N for
pack 'xs' of size M": these wording changes follow the oracle switch,
WP-355). The
substituter then maps the pattern pack to that list (`remap_binding_pack`
after the declaration statement, `DeclOrPack::Pack`), so later expansions
expand over the new bindings. If the decomposition failed, the pack goes
into `failed_locals` and its uses fail silently (already diagnosed). A
pack in a generic lambda whose levels are retained stays a pack, and
`xs...[I]` indexed by the lambda's own parameter keeps its pattern (§7.4).
A binding pack named outside an expansion is diagnosed in the definition
like any pack (below). Tests `tests/conformance/dcl.struct.bind/pack*.cpp`,
`run-pack-fold.cpp`.

**Friend packs** ([class.friend]/3, [temp.variadic]/5.9, P2893, WP-359):
`friend Ts...;` is a `Friend` of type `PackExpansion(Ts)`
(`act_on_friend_type`); `friend_decl` in the substituter expands it into
one `act_on_friend` per element. A friend type or function naming an
unexpanded pack outside an expansion is `sema_unexpanded_pack`
("friend declaration contains unexpanded parameter pack 'Ts'"); a `...`
whose pattern names no pack is "pack expansion does not contain any
unexpanded parameter packs". The friend-type-specifier-list parse is in
spec/parse.md.

An unexpanded pack is the `UNEXPANDED_PACK` bit on types, expressions and
template args. Every *full-expression*, declarator, base-specifier and
template argument outside an expansion context is checked. A violation is
"expression contains unexpanded parameter pack 'T'" (or "declaration
type …", or "base type …"). The check walks the node, pruning on the
flag, to name the first pack.

In a template definition outside any lambda (WP-367),
`Sema::diagnose_unexpanded_written` checks an expression statement, a
`return` operand and a condition ("expression contains …", at the
expression's start; the expression becomes a `RecoveryExpr`), and a
variable's initializer ("initializer contains …", at the `(`/`{` of a
parenthesized or braced initializer, else the expression's start; the
variable becomes invalid and its initializer is dropped). Inside a lambda
body the check is deferred to the enclosing expansion (below), as clang's
`LambdaScopeInfo::ContainsUnexpandedParameterPack` does. An implicit
capture never makes the `LambdaExpr` unexpanded: a pack the body names
inside an expansion or a pack index is captured as a whole. Test
`tests/conformance/temp.variadic/p12-unexpanded-full-expr.cpp`,
`dcl.struct.bind/pack-unexpanded.cpp`.

A declarator's type naming an unexpanded pack (WP-371;
`Sema::diagnose_unexpanded_declarator`, clang `HandleDeclarator`) is one
error at the declarator-id: "data member type contains …" for a
non-static data member, "non-type template parameter type contains …"
for an NTTP, else "declaration type contains …" (variables, pointers,
functions, typedefs, alias-declarations, parameters, special members and
conversion types). `type_packs` walks member pointers and dependent-size
array and vector bounds. The declaration becomes `INVALID`, so a use of
it is silent (`recover_err`). The error sets `Sema::declarator_type_invalid`,
which the parser takes after each declarator: as with clang's sticky
`Declarator::InvalidType`, the rest of a block- or namespace-scope group
is parsed but not declared, so `Ts c = 1, d = 2;` also gets "expected ';'
at end of declaration" at the second `=`; a member group diagnoses every
declarator. A default argument (function or template) naming an
unexpanded pack is "default argument contains …" at the expression's
start (a type template parameter's at the parameter), and the default is
dropped; an in-class default is checked when its delayed parse ends.
Inside a lambda body none of these apply (the lambda carries the pack).

Inside a lambda (WP-392), a pack declared by the lambda itself (a
generic lambda's `auto...` parameter pack, a pack of its own template
head, or an init-capture pack) is not carried by the lambda: it is
diagnosed where it is used, with the same wording and location as outside
a lambda. `Sema::split_lambda_packs` partitions the packs an operand
names into the lambda's own (declared in scopes at or after the lambda's
`scope_base`) and the enclosing ones; only the enclosing ones are
recorded on the lambda (`lambda_unexpanded`, once its body is open).
The sites, as clang:
- "expression contains …": a full-expression in the body
  (`diagnose_unexpanded_full_expr`, from `act_on_full_expr_at`), a
  `switch` condition, a range-for range (at the `{` of a braced range),
  a `case` value; an unexpanded range drops the loop
  (`act_on_for_range_error`) and its initializer is not re-diagnosed
  (`Sema::discarding_init`);
- "static assertion contains …" at the condition's start, which then
  fails silently (also outside a lambda);
- "default argument contains …" for a default argument of the lambda's
  own parameters (`diagnose_unexpanded_default_arg`; `sizeof...(xs)` is
  an expansion and fine);
- "exception type contains …" at the declarator-id (or the type) of a
  handler (`diagnose_unexpanded_type_written`; the handler type becomes
  `int`). A `...` in the handler's direct declarator is parsed
  (`Parser::catch_declarator`, clang `ParseDirectDeclarator`) and is
  "only function and template parameters can be parameter packs" at the
  ellipsis, before that check
  (`except.handle/exception-decl-ellipsis.cpp`);
- "expression contains …" at the operand of a computed `goto *e`
  (`act_on_indirect_goto`), before its conversion to `const void *`; the
  statement is dropped (`stmt.goto/computed-goto-unexpanded-pack.cpp`).
  Instantiation substitutes the target and converts it again
  (`subst/stmts.rs`); an `AddrLabelExpr` in a template's body is
  instantiation-dependent, so it is rebuilt for the specialization's
  label rather than reused from the pattern (WP-444);
- "initializer contains …" for a local variable's initializer.
Outside a lambda these sites check every unexpanded pack (template
definitions); during instantiation they do not run. Tests
`expr.prim.lambda/errors-lambda-local-pack-unexpanded.cpp`,
`temp.variadic/p12-unexpanded-statement-sites.cpp`. Not done: a `...`
in any other non-parameter declarator (`T... x;` in a block, a member,
a variable template). clang reports the same error there; kiln reports
"expected unqualified-id". Also `catch (T(...))`.
At instantiation an `INVALID` pattern declaration is not instantiated:
its `DeclStmt` fails silently and ends its compound statement (clang's
`SubstDecl` / `TransformCompoundStmt`), so no later statement, such as a
`static_assert` after `int a[-1];`, is checked. Tests
`temp.variadic/p12-unexpanded-declarator-type.cpp`,
`p12-unexpanded-declarator-group.cpp`.
The same holds for a declaration statement whose declaration does not
instantiate at all: a `static_assert` whose condition fails to substitute
(not one that is false) and a variable whose type fails to substitute (not
one whose initializer fails; an alias whose type fails also continues).
Its diagnostic is the only one, and the rest of the compound statement is
skipped (`subst/stmts.rs` `DeclStmt`; libc++'s
`static_assert(sizeof(_Tp) >= 0, ...)` guards in `allocator::allocate`;
`temp.inst/failed-decl-stmt-ends-block.cpp`, WP-418).

A lambda whose body names an enclosing template's pack in a
full-expression or a declaration (`[] { A x{}; return sizeof(x); }()...`)
is itself unexpanded: its `LambdaExpr` carries `UNEXPANDED_PACK`, so the
enclosing expansion takes it as part of its pattern. While the
expansion probes for packs (§7.2), the first pack found ends the
substitution of the lambda's body (no statement error is reported
for the probe).

**Capture packs** ([expr.prim.lambda.capture]/17, WP-292). A capture
pack expands the packs of its own initializer, so the `LambdaExpr`
takes its initializer's dependence without `UNEXPANDED_PACK`
(`AstBuilder::lambda_expr`).
  - A simple-capture pack `[ts...]` / `[&ts...]` instantiates to one
    capture per element of the instantiated function parameter pack. In
    keep mode (its pack is a generic lambda's, retained while the
    enclosing template is instantiated) it stays a capture pack of the
    rebuilt parameter pack (`templ/subst/lambda.rs` `lambda_captures`).
  - An init-capture pack `[...xs = init]` / `[&...xs = init]` checks that
    `init` names an unexpanded pack ("pack expansion does not contain any
    unexpanded parameter packs" at the `...`) and declares a `Var` of
    type `PackExpansion(auto)` / `PackExpansion(auto &)`; its
    initializer stays the pattern, undeduced (clang's init-capture pack
    `VarDecl`). `is_pack_decl` counts it as a pack, so `sizeof...(xs)`
    and `xs...` in the body work. Instantiation expands the pattern
    (`expand_pattern_at` at the `...`): each element becomes an ordinary
    init-capture named `xs`, deduced and initialized as one, and the
    pattern's variable maps to `DeclOrPack::Pack` of the new variables.
    An unexpanded result (keep mode) rebuilds the init-capture pack.
  - Instantiated init-captures carry the pattern's variable in
    `LambdaCaptureSpec::var` and skip the duplicate-name check (the
    pattern was checked; an expanded pack's elements share a name). Each
    pattern init-capture maps positionally to its new variables.
  - An init-capture `[x = ts]` naming an unexpanded pack outside an
    expansion makes its lambda unexpanded, so the enclosing statement is
    diagnosed ("expression contains unexpanded parameter pack 'ts'", as
    clang).
  - Lambda-local packs (WP-379, clang's `LambdaScopeInfo::LocalPacks`):
    in a lambda body, the packs a full-expression, initializer or
    declarator type names outside an expansion
    (`Sema::lambda_unexpanded`) are split by where they are declared. A
    pack declared in one of the innermost lambda's own scopes (an
    init-capture pack, a parameter pack, a template parameter pack of a
    generic lambda) is diagnosed at that site, as outside a lambda, even
    in a non-template function (`[](auto... as) { int x = as; }`); the
    others are recorded on the lambda scope (`LambdaScope::packs`), which
    makes the lambda-expression unexpanded. A finished lambda's recorded
    packs (`Sema::lambda_packs`), its call operator's type and its
    non-pack captures are what `expr_packs` reports for the
    lambda-expression, so a nested lambda naming an enclosing lambda's
    pack (`[...xs = ts] { [y = xs] {}; }`) is diagnosed in the enclosing
    body with that pack's name. A use of an init-capture pack in the body
    has a dependent type (the pattern's `auto` is deduced at
    instantiation; `capture.rs`), so an init-capture initialized from it
    is not itself a pack. Not checked during instantiation. Test:
    `expr.prim.lambda/errors-init-capture-unexpanded-pack.cpp`. Declarator types are checked as above. The sites checked
    are an `alignas` operand and the statements and initializers above. `Sema::diagnose_unexpanded_pack`
    (`templ/pack.rs`, emits `sema_unexpanded_pack`; WP-302) is the
    reusable collector and diagnostic for the remaining sites.

**By-copy captures of template-local classes** ([expr.prim.lambda.capture]/10,
/15, [temp.dep.type]/7, WP-332). In a template definition, a local class
of the template or a member class of the current instantiation
(`DEPENDENT_CTX`) declares no implicit special members (clang
`CanDeclareSpecialMemberFunction`). A by-copy capture (simple or
implicit) or a structured binding of an object of such a class keeps the
plain reference as its initializer (`init_subobject_from`), and an
init-capture of such a dependent class type keeps its initializer
unconverted, as a variable does (`defer_dependent_init`). The copy is
selected and checked when the template is instantiated. Test:
`expr.prim.lambda/libstdcxx-copy-capture-local-class-in-template.cpp`
(libstdc++ pstl/algorithm_impl.h:1978).

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
   lengths (2 vs. 3)". This is SFINAE-able. When exactly one of the two
   is an *outer* pack (a pack kept from an earlier substitution,
   `SubstTemplateTypeParmPack` / `SubstNonTypeTemplateParmPackExpr`, or
   one of a shallower depth than the other), clang's multilevel text is
   used instead, naming the inner pack: "pack expansion contains
   parameter pack 'U' that has a different length (2 vs. 1) from outer
   parameter packs" (outer length first). For an explicitly specified
   pack that deduction may still extend (`PackName.partial`), clang's
   "(at least 3 vs. 2)" variant is used, with the explicit length first;
   `deduce_call` checks this before deducing (`check_partial_pack_lengths`,
   WP-161).
4. **Expand.** For k in 0..N, substitute the pattern with
   `pack_index = Some(k)`. Restore the previous index afterwards. Inner
   expansions keep their own packs untouched.
5. **Splice** the N results into the enclosing list: template args, call
   args, initializer list, bases, mem-initializers, `UsingPack`
   members, or capture list.

**Partial substitution** ([temp.variadic]/7, [temp.inst]/3). A pattern
may expand a pack of the level being substituted together with a pack of
an inner, retained level. For example, `Conv<U, T>...` in a member
template of `Tup<T...>`, where T is substituted and U is not. When an
element cannot be formed, `PackState::partial` is set and the expansion
is redone in keep mode (`PackState::keep`). In keep mode, a substituted
type parameter pack becomes `SubstTemplateTypeParmPack {associated,
index, pack}`, which is dependent and unexpanded, and the result is
`PackExpansion(pattern')`. A later substitution that expands the
pattern takes element *i* of the recorded pack (clang's
`SubstTemplateTypeParmPackType`).

A substituted non-type pack is kept the same way, as
`SubstNonTypeTemplateParmPackExpr {param, pack}` (value- and
instantiation-dependent, unexpanded; its type is the first element's).
`All<O<T, I>...>` in a member alias of `Imp<Seq<I...>>` thus keeps `I`
until `T` is supplied (libc++ `<variant>` `__all_overloads`). Both kept
forms count as outer packs for the length check (step 3) and print as
their parameter's name (`P<T, U>...`).

A function parameter pack whose type is partially substituted
(`P<T, U>... ps` in a member template of `S<T...>`) stays one pack
parameter of type `PackExpansion(P<SubstPack(T), U>)`. Deduction against
it substitutes the kept pack element by element: while deducing element
*i* of the expansion, a `SubstTemplateTypeParmPack` in P is matched as
element *i* of its pack. kiln's [temp.deduct.call]/4 "deduced A"
check (§6.1) skips pack parameters, so an element mismatch is reported as
"could not match 'P<T, U>' against 'P<char, long>'" where clang says
"deduced type 'P<int, [...]>' of 1st parameter does not match adjusted
type 'P<char, [...]>' of argument".

A reference to an instantiated function parameter pack met in keep
mode outside any element (the enclosing function template's `idx` in the
body of a generic lambda, `[&]<size_t... P>(seq<P...>) { return ((idx *
s[P]) + ...); }`) cannot expand yet: the lambda's own pack `P` is
retained. It becomes a `FunctionParmPackExpr {param, params}` (clang's
`FunctionParmPackExpr`): an lvalue of the pattern's type,
type-, value- and instantiation-dependent and unexpanded, whose
`params` list holds the instantiated `ParmVar`s. Building it captures
each `ParmVar` at the reference ([expr.prim.lambda.capture]/8, clang
`MarkFunctionParmPackReferenced`). When the generic lambda's call
operator is instantiated, a `FunctionParmPackExpr` is a pack of N
elements for the length check (step 3) and expands to a reference to
element *i*.

The fold and expansion patterns detect a partial element through
`element_expr` (the expression analogue of `element_ty`): an element
that, after substitution with a pack index, still names an unexpanded
pack is rebuilt in keep mode as one unexpanded expansion. A
`PackExpansionExpr` pattern met in keep mode is still "not yet
implemented". An expression pack expansion in a template argument list
uses the same detection (`expand_arg` → `element_expr`, WP-669):
`B<P<Ts, Us>::value...>` in a member template of `A<Ts...>`, met while
the class is instantiated and only `Ts` is known, stays one partially
substituted expansion until the member template's `Us` is supplied
(`tests/conformance/temp.variadic/p5-partial-expr-expansion-member-template.cpp`).

**An empty first pack** ([temp.variadic]/7, WP-802). The probe stops at
the first pack it reaches, so when that pack is empty no element is ever
substituted and the expansion's other packs would go unchecked. Instead
the pattern is substituted once more in keep mode (the *length sweep*,
`PackState::sweep`, diagnostics captured and discarded): every
substituted pack it reaches is kept, and a non-empty one is the length
mismatch of step 3 (reported once the sweep ends, SFINAE-able as
always), so `all<is_convertible(U, T)...>` with `T` = {} and `U` =
{int} is a deduction failure and `is_constructible<tuple<>, int>` is
false. A sweep element (`element_ty` / `element_expr`) that still names
a pack by declaration names one of a retained level, and the expansion is
rebuilt partially substituted as above instead of becoming empty. A sweep
that fails for another reason (a lambda body, a pack it cannot keep)
leaves the expansion empty, as before
(`tests/conformance/temp.variadic/expansion-empty-first-pack-checks-other-packs-run.cpp`,
`p7-empty-first-pack-length-mismatch.cpp`).

A converted argument pack inside an argument list being substituted is
rebuilt element by element. This happens for a partial
specialization's `<U>` against `class... Up`, which is stored as the
pack `{U}`.

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
length stored. It is not dependent. Its dump matches clang's. A pack
bound to elements some of which are pack expansions (an alias
template's `A` := `{int, Ts...}`) has no length yet: the substituted
expression is the count of the other elements plus `sizeof...` of the
first pack each expansion names (`1 + sizeof...(Ts)`), still
value-dependent, so a template-id it selects (`OrImpl<sizeof...(A) !=
0>`) is not formed early (`size_of_partial_pack`, WP-435; test
`temp.variadic/sizeof-pack-bound-to-expansion.cpp`).

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
- Before C++26 a pack index warns `parse_ext_pack_indexing` ("pack
  indexing is a C++2c extension", `-Wc++26-extensions`) at the pack's
  name (clang `ext_pack_indexing`).
- An index that is neither value-dependent nor constant is diagnosed in
  the definition (`pack_index_may_be_constant`): "array size is not a
  constant expression" with the evaluator's notes, clang's wording. The
  expression recovers without its pattern. Clang does not check the type
  form there, nor does kiln.
- An index that is still value-dependent after substitution (a generic
  lambda's or member template's own parameter, `[&]<int I> { return
  ts...[I]; }`) keeps its pattern: the substituter rebuilds the pattern in
  keep mode (`kept_pack_pattern`, `templ/pack.rs`), so a known pack becomes
  `FunctionParmPackExpr` / `SubstNonTypeTemplateParmPackExpr` /
  `SubstTemplateTypeParmPackType` and the inner instantiation selects the
  element. Tests `templates/packs/temp.variadic-pack-index-dependent.cpp`,
  `-not-constant.cpp`, `-ext.cpp`, `dcl.struct.bind/pack-index-generic-lambda.cpp`.

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
    (`bool`) is fixed at construction. "Non-dependent" means no argument
    is type- or value-dependent (clang `anyDependentTemplateArguments`):
    an argument that is only instantiation-dependent (`TC<N>` with `N` a
    `constexpr` local of a function template) is checked at the
    definition. The concept is checked with each such argument replaced
    by its canonical form (`settle_inst_sugar`), so its atoms substitute
    concrete arguments and a nested concept-id or class-template member
    (`IS<T, U>::value`) is evaluated, not assumed satisfied as a
    dependent atom would be
    (`temp.constr.atomic/instantiation-dependent-local-constant.cpp`).
- **The associated constraints of a templated entity**
  ([temp.constr.decl]/3) are the conjunction, in this order, of:
  1. each template parameter's immediately-declared constraint, in order;
  2. the template-head's requires-clause;
  3. the trailing requires-clause.
  Satisfaction checks the conjuncts in this order and stops at the first
  unsatisfied one, so a type-constraint guards substitution into the
  requires-clause. The invented parameters of an abbreviated template are
  template parameters, so their constraints come in step 1, before the
  requires-clause, as in clang 19. The standard (/3.3) and GCC put them
  after it.

### 8.2 Normalization and atomic-constraint identity

```rust
pub struct NormId(NonZeroU32);
pub enum NormNode {                         // 16 B, hash-consed in NormTable
    Atomic { expr: ExprId, mapping: ArgListId },   // ExprId of the expression as written; mapping canonical, used parameters only
    And(NormId, NormId), Or(NormId, NormId),
    Fold { and: bool, pattern: NormId, packs: u32 }, // P2963 fold expanded constraint; packs = interned pack set
}
```

`normalize(e, mapping)` follows [temp.constr.normal]:
- `(E)` normalizes to `E`.
- `E1 && E2` gives `And`; `E1 || E2` gives `Or`.
- A concept-id `C<A…>` normalizes `C`'s constraint expression with the
  parameter mapping substituted by `A…`. The frame is
  `ParameterMappingSubstitution`; a failure there makes the program
  ill-formed (clang's error). The mapping is `A…` shaped to `C`'s
  parameters: when `C`'s last parameter is a pack, the arguments for it
  are grouped into one pack argument ([temp.arg.general]/4), so `C<F, T, T>`
  written directly and `C<F, A...>` reached by substituting `A := {T, T}`
  give the same mapping (and identical atoms).
  Before the mapping is shaped, a concept-id written with fewer arguments
  than `C` has parameters is completed with `C`'s default template
  arguments, each substituted with the arguments before it
  (`concept_default_args`, [temp.arg.general]/4, [temp.param]/12, WP-315).
  Substitution runs under `ConstraintSubstitution`; if a default fails
  there, or a written argument is a pack expansion, the list stays as written.
  So `K<T>` with `template <class T, class U = int> concept K` and
  `K<T, int>` give the same mapping. Without this, the atoms of the
  two spellings would differ and two candidates whose constraints
  differ only in that spelling would be ambiguous instead of ordered
  ([temp.constr.order]; libc++'s `indirectly_comparable` with
  `projected<I, identity>`). Test:
  tests/conformance/temp.constr.order/concept-default-argument-parameter-mapping.cpp.
- Anything else gives an `Atomic` node.

**Identity** ([temp.constr.atomic]/2). Two atomic constraints are
identical iff:
- they are the *same expression in the source*, i.e. the same `ExprId`;
  and
- their mappings are equivalent, i.e. the same canonical `ArgListId`
  (comparing dependent args by `ProfileId`).

The mapping holds only the parameters the expression names
([temp.constr.atomic]/1; clang's `MarkUsedTemplateParameters`): when an
atom is formed under a concept's mapping (`intern_atom`), the arguments of
the concept's parameters the expression does not name are null. So
`sizeof(T) > 1` of `C<T, int>` and of `C<T, long>` is one atom, and
`requires C<T, long> && true` subsumes `requires C<T, int>`. The used set
(`used_template_params`, `templ/concepts/used.rs`, cached per expression
and depth) is a whitelist walk over expressions, types, nested-name-
specifiers, template names and arguments. Every kind is listed (no
fallback arm, WP-776): besides each expression's children, type, written
template arguments and qualifier, it walks what a node holds elsewhere
(a cast's written type, `new`'s allocated type, a pseudo-destructor's
type, trait and `sizeof`/`alignof`/`typeid`/`offsetof` operands, pack
indexing, `va_arg`'s type, dependent member and dependent-scope names
including conversion-function and constructor/destructor names, a
reflect-expression's type, declaration or template), and every type
kind (`typeof`, unary transforms, `auto` with its type-constraint,
deduced template specializations, dependent vectors and `_BitInt`, pack
indexing) and every nested-name-specifier and template-name kind,
reflection splices included (`[: R :]::x`, `typename [: R :]`,
`[: R :]`, a spliced template name). A function type's exception
specification is walked too (`noexcept(E)`, a dynamic list).

A lambda-expression is walked as clang's `RecursiveASTVisitor` does
(WP-821, `used_in_lambda`): its capture initializers and the types of
its captured variables, its call operator's template parameter list
(type-constraints, non-type parameter types, defaults, the
requires-clause), its parameters (types and default arguments), its
trailing requires-clause, its body and the call operator's type
(return type, `noexcept`); its closure type is not walked through the
`LambdaExpr`'s own type. A statement walk (`used_in_stmt`) is exhaustive
over every statement kind, and the declarations a body holds are
walked by `used_in_local_decl`: variables, bindings, typedefs and
aliases, `static_assert`, local enumerations (underlying type,
enumerator initializers) and local classes (bases, members, member
function bodies). A local class or enumeration, and a closure type,
named later in the body are then known (`Used::locals`, the lambda
itself), so a type naming them adds nothing more; a statement
expression walks its compound statement, and an expansion statement
walks its initializer, declaration and body (expanded or not). An
expansion statement's selector names the statement's own synthetic
parameter, deeper than any concept parameter, and adds nothing.

The whole mapping is kept (`None`) only for a dependent member class or
enumeration of a class template (it depends on enclosing parameters it
does not name; a dependent specialization of a namespace-scope
template walks its arguments), an unresolved using-declaration, an
invalid type, a declaration kind a body cannot hold (a friend, a
using-declaration) and a parameter index past 63 (the set is a `u64`
mask); that is never looser than the wording. Tests
`temp.constr.atomic/p1-used-parameter-mapping.cpp` (WP-771),
`p1-used-parameter-mapping-kinds.cpp` (WP-776; clang 19, 22 and GCC
16.1 agree), `p1-used-parameter-mapping-splice.cpp` (WP-776; GCC 16.1
agrees, `skip(clang)`), `p1-used-parameter-mapping-lambda.cpp`
(subsumption through lambdas, statement expressions, local classes and
enums, closure types; clang 19 agrees), `p1-used-parameter-mapping-
lambda-cache.cpp` (the satisfaction cache keyed by the parameters a
lambda names, and the note text; clang 19 agrees) and
`p1-used-parameter-mapping-expansion.cpp` (expansion statements; GCC
16.1 agrees) (WP-821). GCC 16.1 never treats two lambda atoms as
identical, so it is no subsumption oracle for them.

The kept arguments are canonical ([temp.constr.atomic]/2, WP-776,
`written_mapping_arg`): a non-dependent expression argument that
substitution keeps as written (`1` of `C<1, int>` in a template's
requires-clause) is its value converted to the parameter's type when
the parameter is integral or an enumeration (`C<2 - 1, long>` maps
alike). Any other non-pack parameter of a non-dependent type (a
pointer, a class type, a reference, `auto`) maps the argument converted
as a template argument ([temp.arg.nontype]/2) in a SFINAE frame, so
template-argument-equivalent values ([temp.type]/2) map alike: `&x` and
`&(x)`, `S{1}` and `S{0 + 1}` (WP-821, `converted_mapping_arg`).
Otherwise, or when the conversion fails, the argument maps by its
profile. A reflect-expression profiles by its value once known, `^^T`
of a template type parameter by its canonical type, and any other
dependent reflection (`^^A<T>`, `^^T*`, `^^typename A<T>::type`,
`^^A<T>::x`, `^^TT`) structurally by its operand kind and the operand's
dependent profile (spec/types.md §7.2, WP-821), so the same reflection
written in two templates maps alike. Tests
`temp.constr.atomic/p2-non-dependent-argument-identity.cpp` (clang 19,
22 and GCC 16.1 agree), `p2-non-integral-argument-identity.cpp` (clang
19 agrees), `p2-dependent-reflection-identity.cpp` and its `-neg`
sibling (GCC 16.1 agrees, `skip(clang)`) (WP-821).

Hash-consing therefore makes identical atoms the **same `NormId`**.
`normal_forms: DeclId → NormId` caches each entity's associated
constraints, built under the `ConstraintNormalization` frame.

**Fold expanded constraints** ([temp.constr.fold], P2963R3, WP-771;
`normalize_fold_expanded`). A `&&`/`||` fold whose pattern names
unexpanded type or non-type template parameter packs (no concept
template parameter pack) normalizes to `Fold { and, pattern, packs }`:
the pattern's normal form and the set of packs it expands (each
`depth << 16 | index`, interned sorted in `NormTable::pack_sets`). The
pattern is any constraint-expression over the packs (WP-776):
`(C<Ts> && ...)`, `(C<Ts, int> && ...)`, `(C<Box<Ts>> && ...)`; its
atoms are the pattern's as written, so two folds whose patterns
normalize to the same atoms are one node. The init of `(E op ... op I)` is joined on
its side ([temp.constr.normal]/1.5.2: `(E && ... && I)` is
`(E && ...) ∧ I`).

Under a concept's mapping ([temp.constr.normal]/1.4,
`fold_over_mapping`), the concept's pack parameters the pattern expands
take the elements the mapping gives them: an element that is itself an
expansion `X...` of the constrained declaration's packs gives a `Fold`
over those packs whose pattern is the concept's pattern normalized with
`X` in the parameter's place (`fold_pack_element`); a mapping of
non-expansion elements (`AllC<int, long>`) expands to the conjunction
(disjunction) of the pattern normalized with each element, and a mixed
list (`AllC<int, Ts...>`, `AllC<Ts..., int>`) joins both kinds in
order. So
`AllC<Ts...>` through `concept AllC = (C<Ts> && ...)`, `AllC<Box<Ts>...>`
(also through a forwarding concept, `concept ViaC = AllC<Vs*...>`, or
over a non-type pack, `AllN<(Ns * 2)...>`) and the corresponding folds
written directly give the same nodes. The
immediately declared constraint of `C... Ts` is such a fold. A fold over
a concept template parameter pack (concept-dependent, below), over an
enclosing level's pack, or whose mapped element names packs the walk
cannot identify stays one atom.

*Wording over clang:* clang 22 identifies the elements of a fold reached
through a concept by the concept's pack parameter, not by the mapped
pattern, so its orderings differ where the mapped patterns differ:
`AllA<Box<Ts>...>` against `AllB<Ts...>` (patterns `A<Box<Ts>>` and
`B<Ts>`) is ambiguous by the wording. Kiln follows the wording:
`temp.constr.fold/p1-fold-expanded-concept-mapping.cpp` is `skip(clang)`.
GCC 16.1 does not implement P2963.

Subsumption (§8.4) treats a `Fold` as a leaf: `leaf_subsumes(a, b)` holds
when `a == b` or both are folds with the same operator, compatible pack
sets (the same packs, [temp.constr.fold]/2: "an equivalent unexpanded
pack") and `a.pattern` subsumes `b.pattern`. So `(C<Ts> && ...)` is
subsumed by `(D<Ts> && ...)` when `D` subsumes `C`
([temp.constr.fold] Example). The positional comparison of
[temp.constr.order]/4 compares packs by index. Tests
`temp.constr.fold/p1-fold-expanded-subsumption.cpp` (clang 19 and 22
agree), `temp.constr.fold/p1-fold-expanded-subsumption-concepts.cpp`
(`requires: clang>=22`; clang 19 keeps concept folds atomic), and for
general patterns (WP-776, `requires: clang>=22`)
`temp.constr.fold/p1-fold-expanded-general-patterns.cpp` (the
more-constrained overload wins) and
`p1-fold-expanded-general-patterns-ambiguous.cpp` (incompatible pack
sets or non-subsuming patterns are ambiguous, with the "similar
constraint expressions" notes).

**Concept template parameters** (P2841R7, [temp.constr.normal]/1.4-1.5,
WP-746; `normalize_concept_id`, `normalize_concept_fold` in
`templ/concepts/normal.rs`).
- /1.4.1: a concept-id `C<A…>` whose `C` is a concept template parameter
  is a *concept-dependent constraint*. Normalized without a mapping that
  supplies `C` (a declaration's own associated constraints), it is an
  `Atomic` node of the concept-id as written (identity mapping).
- /1.4.2: under a mapping that supplies `C` (normalizing a concept
  `CC = CT<T>` reached through `CC<U*, C>`), the concept template
  parameter is substituted (`Substituter::template_name`) and, when it
  names a concept, the concept-id normalizes through that concept: the
  normal form of `CC<U*, C>` is the normal form of `C<U*>`, so its atoms
  are `C`'s ([temp.constr.normal] Example 3). clang 22 agrees.
- /1.5.3: a `&&`/`||` fold whose pattern contains an unexpanded concept
  template parameter pack, normalized under a mapping that supplies that
  pack as N concepts (N > 0), normalizes as the expanded
  `E0 op … op EN-1`, `Ei` being the pattern normalized with the i-th
  concept substituted, and the fold's init (if any) joined on its side
  ([temp.constr.normal] Example 4: `all_of<T, A, C>` is `A<T> ∧ C<T>`).
  An empty pack, or a fold the mapping does not fix, is /1.5.3.2's fold
  expanded constraint (above) when its packs are type or non-type
  packs, else one atom.
- /1.5.3 Example 5: a fold in a constraint whose operands contain a
  concept template parameter pack and an unexpanded pack of another kind
  is "fold expression in a constraint contains a concept template
  parameter pack and a template parameter pack of another kind"
  (`sema_fold_constraint_mixed_packs`, at the fold, when the constraint
  is checked as written: `check_constraint_fold`).
- *Wording over clang:* clang 22 keeps concept-pack folds whole in
  subsumption, so it reports Example 4 (and the same through a
  forwarding concept, or with `||`) ambiguous, and it accepts Example 5.
  Kiln follows the wording: `temp.constr.normal/concept-pack-fold-expansion.cpp`
  and `temp.constr.normal/concept-pack-fold-mixed-kinds.cpp` are
  `skip(clang)`; `temp.constr.normal/concept-dependent-substituted.cpp`
  (clang agrees) is compared.
- Atom identity uses the used-parameter mapping (above), so an atom of a
  fold's init (`(CTs<T> && ... && true)`) or of its pattern that names no
  concept parameter (`sizeof(T) > 0`) is the same atom across expansions
  whose concept packs differ ([temp.constr.atomic]/1): `all_t<T, B>`
  subsumes `all_t<T, A>` when `B` subsumes `A`. clang 22 keeps these
  folds whole and is ambiguous;
  `temp.constr.normal/concept-pack-fold-atom-identity.cpp` is
  `skip(clang)` (WP-771).

### 8.3 Satisfaction ([temp.constr.sat])

`check_satisfaction(entity, args, loc) -> SatId` runs under the frame
`ConstraintsCheck`. It evaluates the normal form. For a class template-id
the entity is the declaration lookup finds (`visible_template`, §4.8.1),
whose head the notes name. It evaluates:

- **Atomic.** Substitute `args` into `mapping` and then into `expr`,
  under `ConstraintSubstitution` (SFINAE).
  - Names in the atom that substitution looks up again (unresolved
    callees, §5.4) are looked up from the constrained entity's
    definition context, not from the point of the check: for an entity
    in a namespace (with no `this`), the substitution runs inside
    `in_inst_context(namespace)` (`sat_atom_uncached`, WP-201; libc++
    `ranges::__movable_box` concepts checked from `<string_view>`).
  - Static member functions and function templates of a class template
    pattern named by an unqualified callee (`ok<U...>()`) are not looked
    up again: the callee keeps the declarations and becomes a qualified
    reference (`retained_static_members` → `qualified_callee`), each
    mapped to the member of the substituted specialization by
    `find_instantiated_decl`. This is what makes the implicit deduction
    guide of a constructor (template) work, whose requires-clause is
    checked outside the class ([over.match.class.deduct]/1; libstdc++
    tuple:980 under `views::single(tuple{})`). Test:
    `over.match.class.deduct/libstdcxx-guide-constraint-names-static-member.cpp`.
  - A member function template specialization the pattern already
    selected (`ok<int>()` with non-dependent arguments, resolved at the
    definition) is named again as the same specialization of the
    instantiated member template (`respecialized_member_template`); the
    pattern's own specialization stayed value-dependent. Test:
    `temp.constr.decl/libstdcxx-member-template-call-in-requires-clause.cpp`.
    WP-332. When the member template has a parameter pack that is not its
    last parameter, the substituted arguments (one per parameter, the
    pack one `Pack`) form the specialization directly
    (`structural_member_spec`, `get_function_spec`): re-read flat as
    explicit arguments, the pack would absorb a trailing defaulted or
    deduced parameter's argument ([temp.arg.explicit]/9; `mk()` for
    `template <class... Args, class E = int>` became `mk<int>` with
    `Args = <int>`). Tests: `temp.deduct.call/pack-before-*.cpp`. WP-629.
  - A substitution failure means not satisfied, with the captured
    diagnostic recorded.
  - Otherwise the expression must have type exactly `bool`. Anything else
    is the hard error "atomic constraint must be of type 'bool' (found
    'int')".
  - The value is evaluated by the Oven with `EvalKind::Constraint` (a new
    `EvalContext` kind; Q5).
  - A non-constant result is clang's error "substitution into constraint
    expression resulted in a non-constant expression".
  - A substituted atom that is still value-dependent is *satisfied* only
    while the levels themselves are dependent (a retained level or a
    dependent argument: a partial check during a template definition,
    checked again at instantiation). With concrete levels it is an
    **internal error**, not "not satisfied": [temp.constr.atomic]/3
    gives only substitution failure, `false` and a non-constant result
    as outcomes, and substitution with concrete arguments always yields
    a non-dependent expression, so a dependent one means kiln's
    substituter left the atom incomplete. Treating it as unsatisfied
    would silently pick another overload or reject a valid program; it
    is reported as `sema_unimplemented_template_feature` ("an atomic
    constraint that stays value-dependent after substitution",
    `sat_atom_uncached` in `templ/concepts/sat.rs`), and the fix for any
    reachable case is in the substitution, not this guard. No known
    program reaches it (libc++ M3 gate, WP-325). One did (WP-335,
    libc++'s pstl `__cpp17_destructible` = `requires(_Tp __v) {
    __v.~_Tp(); }`): an implicit member of a non-template class
    declared while a template definition was being parsed (`S x;` in a
    function template body declares `S::~S()`) took `DEPENDENT_CTX`
    from the ambient `dependent_depth`, so `v.~S()` was
    instantiation-dependent after substitution. Implicit members now
    follow their class's dependence (`declare_implicit`, spec/sema-class.md §8).
- **`And` and `Or`** short-circuit from left to right.
- **Context.** Atomic constraints are substituted in the context of the
  constrained entity's declaration (its semantic `DeclContext`, via
  `in_inst_context`), not where satisfaction is checked: an unqualified
  name in a concept is looked up from the concept's namespace even when
  the concept-id is checked inside another one.

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

A concept-id naming a concept template parameter is checked only after
substitution has replaced the parameter by its argument concept
(§2.1.1), so both levels key on the substituted concept: `all_of<C,
Ts...>` with `C = Small` is one `sat_cache` entry and `Small<T<i>>` one
entry per type, shared with every other use. Stress:
`tests/stress/concept-ttp-all-of-10k.cpp` (four 10,000-type checks and
their repeats; the repeats are cache hits).

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

**Eligibility** (P2841R7, [temp.constr.order]/3-4,
`eligible_for_subsumption`, `is_concept_dependent`). Associated
constraints that contain a concept-dependent constraint (an atom that is
a concept-id or fold naming a concept template parameter, §8.2) are not
eligible for subsumption. D1 is at least as constrained as D2 only if
D1's constraints are eligible and subsume D2's (or D2 is unconstrained),
so `more_constrained(P, Q)` is `eligible(P) ∧ P ⊒ Q ∧ ¬(eligible(Q) ∧
Q ⊒ P)`. Two candidates constrained by `C<T>` and `C<T> && A<T>` (`C` a
concept template parameter) are ambiguous, with clang 22's "similar
constraint expressions" notes (a fold atom's note points at its pattern).
*Wording over an example:* [temp.constr.order] Example 3 says the call
selects #1 (`requires A<T>`) "because #2 is not eligible"; the normative
¶4 needs #1's constraints to subsume #2's `X<T>`, which they do not, so
the call is ambiguous. Kiln follows ¶4, as clang 22 does
(`temp.constr.order/concept-dependent-not-eligible.cpp`).

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
| compound `{E} noexcept -> C<A…>;` | `E` substitutes; with `noexcept`, `E` cannot throw; with a type-constraint, `C<decltype((E)), A…>` is satisfied, a pack expansion among `A…` (`-> C<P...>`) expanded to its elements (WP-789) |
| nested `requires B;` | `B` is satisfied (§8.3), under `NestedRequirementConstraintsCheck` |

A `requires`-expression outside any template whose requirement is
ill-formed is a hard error ([expr.prim.req.general]/5, clang).

A parameter pack in the parameter list (`requires(T... t, U u)`) is
expanded like a function parameter pack (§7.2): one parameter per
element, numbered consecutively with the other parameters. When the
pack's expansion names a retained level, it stays one pack parameter.
A requirement whose substituted form is still dependent (it names a
retained level: a generic lambda's `auto` inside an instantiated
function template) is not checked. The requires-expression is then
rebuilt as a dependent `RequiresExpr` with the substituted parameters
and requirements, and evaluated when the remaining level is
substituted. A compound requirement's type-constraint keeps its
invented first argument in that case. The expression is dependent when
a parameter's type or a requirement is ([temp.variadic]/5, WP-398); a
requirement that already fails settles it `false`, and the result then
drops parameters whose types still name a retained level so it is not
substituted again.

The expression is a `bool` prvalue. `RequiresExpr` stores the value plus
the index of the failed requirement.

### 8.6 Constrained parameters, constrained `auto`, abbreviated templates

- **`C<A…> T`.** The type parameter gets the immediately-declared
  constraint `C<T, A…>`, stored as its type-constraint
  (`ConceptReference` in clang's dump).
- **`C auto` in a parameter.** This creates an invented parameter with
  that constraint (§2.6).
- **`C auto x = e;`** checks `C` against the deduced type (§6.3).
- **A constrained NTTP `C<A…> auto V`.** The parameter gets the
  immediately-declared constraint `C<decltype(V), A…>` ([temp.param]/5,
  clang's `AttachTypeConstraint`; a fold `(C<decltype(Vs), A…> && ...)`
  for a pack), stored as its type-constraint, so it is an associated
  constraint: it is checked with the template's other constraints,
  decides partial-specialization matching and, through subsumption,
  ordering (WP-792). The type of `V` takes part in a profile and in the
  /6.2.2 parameter comparison with its type-constraint ignored
  ([temp.over.link]/6.2, clang's `getUnconstrainedType`), so
  `decltype(V)` names one type in `C1<decltype(V)>` and
  `C2<decltype(V)>` of two templates. Other constrained placeholders
  (`C auto&`, `C auto*`, `C decltype(auto)`) are checked against the
  deduced type when the argument is converted. A type-constraint naming
  an earlier parameter (`template<int N, C<N> auto V>`) has its
  arguments substituted with the prior arguments, like clang's
  `TransformAutoType`, so the placeholder is no longer dependent and the
  argument for `V` is deduced and evaluated: `box<7, make()>` and
  `box<7, zero>` are one specialization (WP-792).

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

The clang `ALIAS` keywords `__is_same_as` and `__is_literal_type` build
a `TypeTraitExpr` of the aliased trait (`__is_same`, `__is_literal`),
which diagnostics and dumps print (WP-171); `__is_convertible_to` is a
trait of its own.

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
| `__is_trivially_equality_comparable(T)` | clang 19 (WP-166): not an enum or array; for a class, `const T& == const T&` (built under a SFINAE frame from the TU) selects a defaulted `operator==` whose parameters are `const T&` (or `T` when trivially copyable), recursively for every base and field (reference or enum fields fail; a union fails); then unique object representations without the trivially-copyable requirement | `cls` + `abi` + lookup |
| `__is_trivially_relocatable(T)` | clang 19 (WP-166): a class that can be passed in registers (`canPassInRegisters`, as `__can_pass_in_regs`), or any other complete object type; references, functions and `void` are false. At `-fclang-compat=22` it also warns `sema_deprecated_builtin` (`-Wdeprecated-builtins`) naming `__builtin_is_cpp_trivially_relocatable`, as clang 22 (WP-355), where written and again per instantiation that rebuilds it; at 19 it does not (tests `ext.traits/is-trivially-relocatable-deprecated.cpp`, `-19.cpp`; WP-486) | `cls` |
| `__is_base_of(B,D)` | both are classes (not unions), `D` is complete when the two differ, and `B` is `D` or a base of `D` (cv ignored) | `cls.bases` (S3C) |
| `__is_convertible(F,T)` / `__is_nothrow_convertible(F,T)` | `conv_check(F,T).ok` (and `.nothrow`). WP-131 landed them (and `__is_convertible_to`): `To` void is true only for `From` void; `To` function or array, or `From` void, is false; otherwise the copy-initialization of a `To` from an opaque `declval<From>()` operand under a SFINAE frame, access from the TU; nothrow is `!can_throw` of the result | `conv_check` |
| `__is_constructible(T,A…)` / `__is_trivially_constructible` / `__is_nothrow_constructible` | `init_check(T, A…, Direct)`: `.ok`, `.ok && .trivial`, `.ok && .nothrow`. A reference `T` takes exactly one argument; it still counts as constructible when the reference binds to a temporary created by an implicit conversion (clang 19; only `__reference_constructs_from_temporary` excludes that). WP-131 landed `__is_constructible` and `__is_nothrow_constructible`: a temporary's direct-initialization (value-initialization for no arguments) built on opaque `declval` operands under a SFINAE frame, access from the TU; nothrow is `!can_throw` of the result | `init_check` |
| `__is_assignable(T,U)` / `__is_trivially_assignable` / `__is_nothrow_assignable` | `declval<T>() = declval<U>()` is well-formed (unevaluated), and is trivial / cannot throw. `__is_nothrow_assignable` landed in WP-131 (`!can_throw` of the WP-130 assignment) | `init_check(Assign)` |
| `__is_destructible(T)` / `__is_nothrow_destructible(T)` / `__is_trivially_destructible(T)` | [meta.unary.prop] (`void`, functions and arrays of unknown bound are false); the destructor's uninstantiated noexcept-specifier is instantiated first ([temp.inst]/14, WP-335) | `cls.dtor` |

Two more behave the same way but are only probed, not used, by the M3
headers. They are implemented in M3 because they cost one row each:
`__reference_constructs_from_temporary` and `__is_layout_compatible`.

**WP-166 rows** (`templ/builtins/traits/special.rs`), each to clang 19:
- `__has_nothrow_assign` / `__has_nothrow_move_assign` /
  `__has_nothrow_copy` / `__has_nothrow_constructor` (GCC; deprecated,
  `-Wdeprecated-builtins`): POD (and references for `copy`) answer true,
  `const` or reference operands of `assign` false; otherwise the trivial
  special member answers true, else every matching non-template
  member (copy assignment / move assignment / copy constructor callable
  with one argument / default constructor callable with none) must be
  nothrow, and at least one must exist.
- `__can_pass_in_regs(T)`: `T` must be an unqualified class type
  ("argument 'X' is not an unqualified class type"); the answer is
  clang's `canPassInRegisters` (no non-trivial non-deleted copy/move
  constructor or destructor, and one non-deleted copy/move constructor).
- `__is_bitwise_cloneable(T)`: false for `void`, incomplete and dependent
  types; arrays use their element; classes need every field to be;
  every other type (references, functions) is true.
- `__is_layout_compatible(T,U)` ([basic.types.general]/11): same type
  ignoring cv (array elements included); enums with the same underlying
  type; standard-layout structs whose fields, taken from the class that
  declares them (`getStandardLayoutBaseWithFields`), match pairwise in
  type, bit-field width, alignment and no `[[no_unique_address]]`;
  unions match fields in any order. Incomplete operands other than `void`
  and `T[]` are "incomplete type 'X' where a complete type is required".
- `__is_pointer_interconvertible_base_of(B,D)` ([meta.rel]/5): non-union
  classes; the same class (cv ignored) is true, else `D` must be complete
  and standard-layout with `B` as a base.

**clang 22 rows** (WP-263, `templ/builtins/traits/cxx26.rs`), each to
clang 22.1 `SemaTypeTraits.cpp`. kiln accepts them at every
`-fclang-compat` level, a superset of clang 19 (which lacks them); only
`__has_builtin` follows the level (spec/pp.md "clang compatibility
level"). The exception is `__is_referenceable`, which clang 22 removed:
at level 22 it is an ordinary identifier (the parser's keyword table
omits it) and `__has_builtin` answers 0 for it (WP-394).
- `__builtin_is_virtual_base_of(B,D)` ([meta.rel], P2985; clang 20):
  false unless both are non-union classes; `D` must then be complete
  ("incomplete type 'X' where a complete type is required" plus the
  forward-declaration note, answer false); true when some path from `D`
  to `B` ends in a `virtual` base-specifier (`base_paths`, cv ignored).
  The same class is false.
- `__builtin_is_implicit_lifetime(T)` ([basic.types.general]/9,
  [class.prop]/9; clang 20): completeness of a class operand (arrays
  are not looked through); scalar, array and vector types are true,
  other non-class types false; an aggregate whose destructor is not
  user-provided is true; otherwise the destructor must be trivial and not
  deleted, and some constructor trivial and not deleted (a declared one
  that is eligible, its constraints satisfied ([special]/6, clang
  `isIneligibleOrNotSelected`), or an implicit default, copy or move
  constructor that would be).
- `__builtin_is_cpp_trivially_relocatable(T)` (clang 21; P2786's
  [class.prop] rules without its keywords): element completeness; the
  element type is false when incomplete, true when scalar or vector; a
  class needs no virtual base, every base and every non-reference field
  trivially relocatable, and a destructor that is not deleted; then a
  union with no user-declared copy/move constructor, copy/move
  assignment or destructor is true, and otherwise the class must be
  default-movable: move construction and move assignment from an xvalue
  each select a member that is not user-provided and not deleted (or the
  simple implicit one), and the destructor is not user-provided (one
  defaulted after its first declaration is user-provided, so false).
  The simple implicit copy operation stands in for a move operation
  only when that move operation is not user-declared: a user-provided
  move assignment beside a defaulted move constructor is false.
- **Staged off** (`Status::Owner("WP-263 follow-up")`, so `__has_builtin`
  answers 0 at every level and the headers use their library
  fallbacks): `__builtin_common_type` (a template builtin, like
  `__type_pack_element`), `__builtin_structured_binding_size` (a `size_t`
  value; `TypeTraitExpr` holds a `bool`), `__builtin_invoke`,
  `__builtin_is_within_lifetime` and
  `__builtin_{lt,le,gt,ge}_synthesizes_from_spaceship`. In compile
  modes these are the only names of the probed set whose answer differs
  from clang 22's at level 22; `-E` is unstaged and answers as clang. `__builtin_is_replaceable` and `__builtin_common_reference`
  answer 0 in clang 22 as well.

**Reference-from-temporary traits** (WP-138; libc++ `tuple`'s
`std::tie` constructor uses `__reference_binds_to_temporary`). These are
`__reference_binds_to_temporary(T,U)`,
`__reference_constructs_from_temporary(T,U)` and
`__reference_converts_from_temporary(T,U)`. Each is true iff:
- `T` is a reference;
- `U` is complete and not `void`;
- initializing `T` from an opaque operand of type `U` is well-formed
  (under a SFINAE frame, unevaluated); and
- the initialization materializes a temporary
  (`MaterializeTemporaryExpr` under parens, implicit casts and
  cleanups).

The operand's value category depends on the trait. The C++23 pair takes
a non-reference `U` as a prvalue ([meta.unary.prop]), while
`__reference_binds_to_temporary` takes it as an xvalue (clang 19). At
`-fclang-compat=22` `__reference_binds_to_temporary` is evaluated as
`__reference_constructs_from_temporary` (prvalue `U`) and warns
`sema_deprecated_builtin` naming it, as clang 22 (WP-394). The
`converts` form copy-initializes; the other two direct-initialize.
A prvalue operand of a non-class, non-array type has its cv-qualifiers
dropped ([expr.type]/2), so
`__reference_constructs_from_temporary(int&&, const int)` is true. This
holds at every `-fclang-compat` level, since clang 19's false is a bug
that clang 22 and GCC 16.1 do not have (WP-477;
`builtin.reference-from-temporary-cv{,-19}.cpp`).

A deprecated trait spelling warns where it is written and again at each
instantiation that rebuilds it (under the instantiation note), as clang's
`BuildTypeTrait` (WP-394).

### 9.2 Type transforms (`UnaryTransform` types)

libc++ 19 spells these as `using __remove_cv_t = __remove_cv(_Tp);`.
- The type is `UnaryTransform{kind, operand}` sugar whose canonical type
  is the result. It prints as clang does, e.g. `__remove_cv(int const)`.
- A dependent operand gives a dependent `UnaryTransform`. Substitution
  recomputes it (§5.3). Only a dependent operand defers it (clang
  `BuildUnaryTransformType` tests `isDependentType`): an operand that is
  merely instantiation-dependent (`TC<N>*`, `N` a constant local of a
  function template, reaching an alias substituted with its arguments as
  written at compat 22) is transformed when the type is built
  (`tests/conformance/temp.dep.type/instantiation-dependent-unary-transform.cpp`).

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
| `__builtin_constant_p(e)` | `int`. The Oven returns 1 if `e` (integral, enumeration, floating, pointer or `nullptr_t`, with no assignment, increment, `new`, `delete` or `throw` of its own) evaluates to a constant. A pointer counts only if it is null or the start of a string literal. Other types and arguments with side effects give 0. The argument is evaluated speculatively (`Op::Speculate`): a failure gives 0, but a resource limit still fails the evaluation. Codegen folds the call through the Oven and emits 0 if that fold fails, as clang does at -O0 (WP-130) | Oven, codegen |
| `__builtin_classify_type(e)` | `int`, an integer constant expression whatever `e` is: GCC's type class of `e`'s type (`Ast::gcc_type_class`, clang `EvaluateBuiltinClassifyType`, the same in clang 19 and 22). `e` is checked as a call argument (a bound member function or a builtin name is an error) but not converted or evaluated, so side effects and non-constant calls are dropped. Classes: `void` 0; integer and character types 1; enumerations (scoped too) 3; `bool` 4; pointers, arrays, string literals and functions 5; pointers to data members 7; floating types 8; `_Complex` 9; classes, closures and pointers to member functions 12; unions 13; `_BitInt` 18; vectors 19; `std::nullptr_t`, the SVE types and `std::meta::info` -1. `_Atomic(T)` classifies as `T`. An overload set is accepted and gives -1 (clang reads an unspecified value). The Oven and codegen both fold the call to the constant (WP-419) | Oven, codegen |

**Failed-trait explanation** (clang 22 `DiagnoseTypeTraitDetails`,
`traits/explain.rs`, WP-413). At `-fclang-compat=22` only, a failed
`static_assert` condition and a failed concept atom are checked for a
type trait. The condition is taken after parentheses and implicit casts
are skipped. Three forms qualify: `__trait(T)`, `std::is_X_v<T>`, and
`std::is_X<T>::value` (std may include an inline namespace). For each,
kiln appends clang's explanation notes at the condition's begin location.
Only `is_trivially_copyable` is explained so far. Its notes, in order:
- "T is not trivially copyable";
- "because it is a reference type";
- per base, "because it has a virtual base B", then "because it has a
  non-trivially-copyable base B";
- per member, "…non-trivially-copyable member 'm' of type M";
- "because it has a user-provided destructor";
- "because it has a user provided copy/move constructor/assignment
  operator", in member order;
- "'R' defined here" at the class.

Other traits produce no notes yet; see the WP-413 follow-ups. Tests:
`templates/builtins/meta.unary.prop-trivially-copyable-explain{,-clang19}.cpp`
and `templates/concepts/static-assert-concept-atom-expr{,-clang19}.cpp`.

### 9.5 `__builtin_*` functions used by the M3 headers

These are declared by WP-063's builtin declaration table, with custom
type-checking where marked †. "Oven" means the builtin is evaluable in
constant expressions in clang 19 and therefore in the Oven. WP-074
verifies every Oven bit with a `static_assert` probe against clang 19 and
records it in `TABLE`. Since WP-393 the column also marks the P0533
builtins the Oven folds beyond clang 19 (`abs`/`labs`/`llabs`,
`signbit`, the six comparison macros): `__has_constexpr_builtin` gives
the clang table's answer at the `-fclang-compat` level for every `Yes`
row (spec/pp.md "clang compatibility level").

| Group | Builtins | Oven | Lowering |
|---|---|---|---|
| object model | `__builtin_addressof`†, `__builtin_launder`†, `__builtin_bit_cast`† (keyword form: `T` and the operand must be trivially copyable and the same size; `abi` layout gives the bytes), `__builtin_unreachable` | yes (`unreachable`: a non-constant diagnostic) | intrinsic / no-op |
| allocation | `__builtin_operator_new`, `__builtin_operator_delete` | yes in clang 19; the Oven side is WP-061b's (transient allocation) | `operator new`/`delete` call |
| bits | `__builtin_clz`/`l`/`ll`, `__builtin_ctz`/`l`/`ll`, `__builtin_popcount`/`l`/`ll`, `__builtin_clzg`†/`ctzg`†/`popcountg`† (type-generic, with an optional fallback argument), `__builtin_bswap16`/`32`/`64` | yes | `llvm.ctlz`/`cttz`/`ctpop`/`bswap` |
| memory and strings | `__builtin_memcmp`, `__builtin_memcpy`, `__builtin_memmove`, `__builtin_memchr`, `__builtin_char_memchr`, `__builtin_strlen`, `__builtin_wcslen`, `__builtin_wmemchr`, `__builtin_wmemcmp`, `__builtin_strcmp`, `__builtin_strncmp`, `__builtin_strchr`, `__builtin_wcscmp`, `__builtin_wcsncmp`, `__builtin_wcschr`; `__builtin_strrchr`, `__builtin_strpbrk`, `__builtin_strstr` | yes (for trivially copyable element types); no for `strrchr`/`strpbrk`/`strstr` (clang 19) | libcall or `llvm.mem*` |
| float classification | `__builtin_isfinite`, `__builtin_isinf`, `__builtin_isnan`, `__builtin_isnormal`, `__builtin_signbit`†, `__builtin_isgreater`, `isgreaterequal`, `isless`, `islessequal`, `islessgreater`, `isunordered` | yes (`signbit` and the six comparisons since WP-365, P0533; clang 19 rejects them) | `fcmp` / `llvm.is.fpclass` |
| atomics (WP-154) | `__c11_atomic_*`†, `__atomic_*`† (an `AtomicExpr`, spec/sema.md §5.11) | only `__atomic_always_lock_free`/`__atomic_is_lock_free`/`__c11_atomic_is_lock_free` (spec/oven.md) | `load atomic`/`store atomic`/`atomicrmw`/`cmpxchg`/`fence` (spec/codegen.md §7d) |
| coroutines (WP-171) | `__builtin_coro_resume`/`destroy`/`done`/`promise`/`noop`/`size`/`align`/`frame`/`free`/`id`/`alloc`/`begin`/`end`/`suspend` (`builtin::CORO`, clang 19's types and `noexcept`; an `I` signature argument must be an integer constant expression, `sema_constant_integer_arg_type` at the call, through `check_builtin_ice_args`) | no (the generic note) | not implemented (a codegen error) |
| float constants | `__builtin_huge_val`/`f`/`l`, `__builtin_nan`/`f`/`l`, `__builtin_nans`/`f`/`l` | yes | constants |
| math | `__builtin_exp`/`exp2`/`expm1`/`frexp`/`ldexp`/`pow`/`scalbn`/`scalbln`, each with the `f` and `l` forms | the P0533 set yes (WP-365: `frexp`/`ldexp`/`scalbn`/`scalbln` here, and `ceil`, `fmod`, `fma`, `nextafter`, ... of the other libm rows, spec/oven.md "Floating-point math, P0533"); the rest no (clang 19 evaluates none) | libcall / LLVM intrinsic |
| complex math (WP-237) | `__builtin_cabs`/`carg`/`cacos`/`cacosh`/`casin`/`casinh`/`catan`/`catanh`/`ccos`/`ccosh`/`cexp`/`clog`/`cpow`/`cproj`/`csin`/`csinh`/`csqrt`/`ctan`/`ctanh`, each with the `f` and `l` forms (`builtin::LIBM`; the signature prefix `X` is `_Complex` of the type after it; `cproj` is `const`) | no (clang 19) | libcall through the complex call ABI (spec/abi.md §8.17) |
| source location | `__builtin_FILE`, `__builtin_FILE_NAME`, `__builtin_FUNCTION`, `__builtin_LINE`, `__builtin_COLUMN`, `__builtin_source_location`† (keywords: a `SourceLocExpr`; in a default argument, the location of the call that uses it) | yes | constants; a private `.constant` `__impl` object per location |
| checked arithmetic (WP-270) | `__builtin_add_overflow`†, `__builtin_sub_overflow`†, `__builtin_mul_overflow`† (generic: two integer operands of any types, a pointer to a non-const integer result); the typed `__builtin_{s,u}{add,sub,mul}_overflow` with `l`/`ll` forms (fixed `int`/`long`/`long long` parameters, signed or unsigned) | yes | `llvm.{s,u}{add,sub,mul}.with.overflow` in clang's encompassing width (spec/codegen.md §3) |
| carry (WP-557) | `__builtin_addcb`/`addcs`/`addc`/`addcl`/`addcll` and `__builtin_subcb`/`subcs`/`subc`/`subcl`/`subcll` (`T f(T x, T y, T carryin, T *carryout)`, `T` `unsigned char`/`short`/`int`/`long`/`long long`; fixed parameters, not custom-checked: `x + y + carryin` (`x - y - carryin`) modulo 2^N, `*carryout` 1 when either step wrapped) | yes (spec/oven.md "checked arithmetic") | two `llvm.u{add,sub}.with.overflow`, the flags OR-ed into the carry-out (spec/codegen.md §3) |
| stack allocation (WP-270) | `__builtin_alloca`, `__builtin_alloca_with_align` (the alignment an `I` argument in bits: a power of two, at least 8, at most `INT_MAX`) | no ("subexpression not valid") | dynamic `alloca i8` where the call is |
| printf family (WP-270) | `__builtin_vsnprintf`, `__builtin_snprintf`, `__builtin_sprintf` (signature letter `R`: `restrict`; implicit `format(printf, F, A)`, WP-282) | no | libcall |
| library builtins (WP-282) | `__builtin_assume_aligned`†, `__builtin_object_size`, `__builtin_dynamic_object_size` (the type an `I` argument in [0, 3]) | yes | the pointer plus an `align` assumption bundle; `llvm.objectsize` or a folded constant |
| library builtins (WP-282) | `__builtin_powi`/`f`/`l`, `__builtin_shufflevector`† (a `ShuffleVectorExpr`), `__builtin_verbose_trap`, `__builtin_copysignf16` | `shufflevector` and `copysignf16` (WP-365) yes; the others are "subexpression not valid" | `llvm.powi`, `shufflevector`, `llvm.trap`, `llvm.copysign.f16` |
| `*f128` math (WP-282) | the `__builtin_*f128` forms of the `math` libm rows (`fabsf128`, `copysignf128`, `ceilf128`, ..., `huge_valf128`, `inff128`, `nanf128`, `nansf128`), typed `_Float128` (signature `LLd`) as GCC, whose libstdc++ x86-64 `<cmath>` passes `_Float128*` to `modff128`/`frexpf128`/`remquof128` (clang 19 types them `__float128` on x86-64 and `long double` on AArch64; values convert either way, pointers do not) (WP-361) | `fabs`/`copysign`/`fmax`/`fmin` and the constants yes, through the `l` twin (`f128_twin`), as clang 19; the P0533 forms (`ceilf128`, `fmodf128`, `nextafterf128`, ...) in binary128 (WP-365) | the twin's path at `fp128` (intrinsic or constant); `roundevenf128` `llvm.roundeven`; the rest a libcall of the builtin's own name (`acosf128`) |
| `_FloatN` constants (WP-361) | GCC's `__builtin_huge_valf16`/`f32`/`f64`, `__builtin_inff16`/`f32`/`f64`, `__builtin_nanf16`/`f32`/`f64`, `__builtin_nansf16`/`f32`/`f64`, `__builtin_nansf16b` (kiln ids 2000–2012, `builtin_ids::FLOATN`; signature letters `x` `_Float16`, `k` `_Float32`, `K` `_Float64`, `y` `__bf16`), and `__builtin_nextafterf16`/`f16b` (2013, 2014), which libstdc++ `<limits>`/`<cmath>` name; clang 19 has only some `f16` forms | the constants yes, through their `float`/`double` twin (`f128_twin`) converted to the result type; `nextafterf16`/`f16b` yes (WP-365, spec/oven.md P0533) | constants; `nextafterf16`/`f16b` an inline `i16` select sequence (spec/types.md) |

**Landed (WP-088).** Every row above is implemented in sema, the Oven
and codegen, with these exceptions, each matching clang 19:
the math rows are sema and codegen only outside the P0533 set, which
the Oven evaluates since WP-365 (as do `__builtin_signbit` and the six
ordered comparisons, which clang 19 rejects; spec/oven.md). `__builtin_operator_new`/`_delete` are
implemented in sema and codegen; their Oven side stays with WP-061b
(`__has_constexpr_builtin` answers 0). Probes:
`tests/conformance/expr.const/builtins-*.cpp` (constant evaluation,
`static_assert` against clang 19, plus the compile-fail and run tests),
`tests/codegen/builtins_*.cpp` (IR-diff), and
`bench/oven/builtins-{memcpy,strlen,popcount}.cpp`. Oven behaviour
(byte-level `memcpy`/`memmove` on trivially copyable element arrays,
`bit_cast` through the ABI layout, source-location objects) is in
spec/oven.md "Builtins".

**Landed (WP-140): C string builtins.** The nine `str*`/`wcs*` names in
the memory-and-strings row (`kiln_ast::decl::builtin::STRCMP`..`WCSCHR`, clang's ids)
are used by libc++'s `char_traits` and `type_info`. `strcmp`, `strncmp`,
`wcscmp`, `wcsncmp`, `strchr` and `wcschr` are constant-evaluated with
clang 19's results (narrow characters compare as `unsigned char`, wide
ones in `wchar_t`'s own signedness; a count of 0 is 0 before any read;
`strchr` of a value the element type cannot represent is null, of 0 the
terminator; reading past the end or through a null pointer is clang's
note). `strrchr`, `strpbrk` and `strstr` are libcall-only, as in clang 19:
in a constant expression they are "subexpression not valid". All nine
lower to the C library call (the `wcs*` declarations carry
`memory(read)`); an integer result with constant operands folds. They are
not counted in the 143 header builtins of §13.3, which are the M3
headers' own uses; `TABLE` now holds 91 function rows. Tests:
`tests/conformance/expr.const/builtins-str{,-errors,-run}.cpp`,
`tests/codegen/builtins_str.cpp`.

**Landed (WP-270): checked arithmetic, alloca, printf builtins.**
`builtin::CHECKED` holds the 26 rows (clang 19 ids; `checked_op` maps
an overflow id to its operation and whether it is a typed form). The
generic overflow builtins are custom-checked (`builtin_overflow`, clang
`SemaBuiltinOverflow`): exactly three arguments (the usual count
errors); each argument is decayed; an operand must be an integer type
(integral, `bool`, or an unscoped enumeration; a scoped enumeration,
floating or class type is `operand argument to overflow builtin must be
an integer type ('T' invalid)` at the operand); the result must be a
pointer to a non-const integer type, volatile allowed (`result argument
to overflow builtin must be a pointer to a non-const integer type ('T'
invalid)`, also for `nullptr` and `0`). The call is a `bool` prvalue and
the arguments keep their own types. The typed forms are ordinary calls
(conversion errors as for any parameter). `__builtin_alloca_with_align`'s
alignment, after the integer-constant check, must be a power of two
(`requested alignment is not a power of 2`), at least 8 (`requested
alignment must be 8 or greater`) and at most 2147483647 (`requested
alignment must be 2147483647 or smaller`), reported at the callee
(`check_alloca_align`, clang `SemaBuiltinAllocaWithAlign`). clang's
implicit `format` attributes on the printf rows landed with WP-282 (see
§5.4 of spec/sema.md, "Format strings"). Tests: `tests/conformance/ext.gnu.builtins/`
(`overflow-args`, `overflow-constexpr`, `overflow-constexpr-invalid`,
`overflow-run`, `alloca-args`, `alloca-run`, `printf-builtins-run`,
`libstdcxx-gnu-builtins`), `tests/codegen/builtins_{overflow,alloca,printf_lib}.cpp`.

**Landed (WP-282): library builtins.** The three "library builtins" rows
and the `*f128` row above (`builtin::F128`, clang 19 ids; `TABLE` rows
with clang 19's `__has_constexpr_builtin` answers). The argument checks
are spec/sema.md §5.4 ("Library builtins", "Format strings"); codegen
spec/codegen.md §3; Oven spec/oven.md "Landed (WP-282)". Not provided:
`__fp16`/`_Float16` as keywords (so `copysignf16` is only reachable
through `float` arguments), and `-Wformat` beyond the subset in §5.4.
Tests: `tests/conformance/ext.gnu.builtins/` (`library-builtins-args`,
`library-builtins-dependent`, `library-builtins-constexpr`,
`object-size-constexpr`, `format-printf`) and `tests/codegen/builtins_`
`{assume_aligned,object_size,powi,shufflevector,f128}.cpp`.

**Source location.** Sema computes the values once, when it builds the
`SourceLocExpr` (spec/ast.md §5.8): `LINE`/`COLUMN` from the presumed
location, `FILE` as written (the default `-fkiln-source-path-style=clang`;
GCC 16.1's simplified absolute path of an included file under `gcc`,
spec/driver.md, WP-636), then `-fmacro-prefix-map=`, and `FILE_NAME` as
its last path component,
`FUNCTION` from the enclosing function's name (empty at namespace scope),
and `__builtin_source_location()` as a frozen `std::source_location::__impl`
built from that record's layout. When a default argument containing a
`SourceLocExpr` is used by a call, sema records the call's values on the
`CXXDefaultArgExpr`; likewise a use of a default member initializer
containing one records, on the `CXXDefaultInitExpr`, the values of the
constructor's name (the class name for an implicit constructor) or of the
`}`/`)` of an aggregate initialization, with the constructor or the
enclosing function as the function (clang `CXXDefaultInitExpr::
getUsedLocation`/`getUsedContext`). For a parenthesized aggregate
initialization by a C-style cast `(S)x` that is the operand's last token
(`Sema::expr_end_loc`, clang `getEndLoc`; WP-182): a stored closing token
(a call's or a parenthesized operand's `)`, a `}`), else the end of the
last operand of an implicit node, cast, prefix/binary operator or
conditional, else the node's own location (a name, literal, member name,
postfix operator). A built-in subscript records its `]` as its end
location (`set_subscript_end`, from the parser and from substitution's
pattern; WP-203), so `(S)a[i]` takes the `]`. The Oven and codegen read them from
the outermost such default argument or member initializer in scope, and
the codegen prepass evaluates the immediate invocations of such a member
initializer per use (`fold_immediate_in`). Substitution drops the trailing
`CXXDefaultInitExpr` fillers of a braced list; the rebuilt list adds them
in the instantiation's context. `std::source_location::__impl` must be a
complete, standard-layout, non-union class without bases whose only
fields are `_M_file_name`, `_M_function_name` (`const char *`), `_M_line`
and `_M_column` (integral); otherwise clang's
`sema_std_source_location_impl_not_found`/`_malformed` errors.

The counts are 49 type predicates (§9.1), `__array_extent`, 15 transforms,
2 builtin templates and 76 `__builtin_*` names (2 of them in §9.4). That gives 143
builtins in total (§13.3). `TABLE` also holds the 6 source-location
names, for 82 function rows (91 with WP-140's string builtins, §9.5).
WP-150 adds 26 function rows (`builtin::INT_MEM`: the GCC integer builtins,
`bcmp`, `wmemcpy`/`wmemmove`, `memset`, `mempcpy`; folding status in
spec/oven.md "Builtins").

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
| `ClassInstantiation` | hard | in instantiation of template class '%0' requested here (`%0` is the specialization's own type, canonical and scoped: `'ns::S<unsigned long>'`) |
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
| `LambdaSubstitution` | hard | while substituting into a lambda expression here (WP-163; see below) |
| `DeclaringSpecialMember`, `DefiningSynthesizedFunction`, `RewritingOperatorAsSpaceship` | — | owned by S3C/S3O; listed here so that they count toward depth and elision |

A lambda body substituted inside a specialization gets its own
`LambdaSubstitution` frame (clang `TemplateInstantiator::TransformLambdaBody`,
WP-163):
- **Entity and point.** The entity is the new call operator, and the
  point is the body's `{`.
- **No instantiation, no SFINAE.** The frame is not an instantiation
  (`is_instantiation` is false), so it does not count as a specialization
  of its own. It is hard (`sfinae = Some(false)`): CWG2672 says a lambda
  body is never in the immediate context.
- **The note chain.** An error in the body prints "while substituting
  into a lambda expression here" at that `{`, then the enclosing chain
  (`in instantiation of function template specialization 'f<int>'
  requested here`, …).
- **Nesting.** Each nested lambda adds one note.
- **Several errors in one body.** The context prints once, before the
  first of them, as for any frame (§10.2).

Tests: `temp.inst/lambda-body-substitution-notes.cpp` and
`expr.prim.lambda/lambda-body-substitution-failure.cpp`. Both are
strict-diags and generated from clang 22.

A specialization in a note (and in the other diagnostics naming a
specialization declaration: an ambiguous or late explicit
specialization, a redefinition) prints as clang's
`getNameForDiagnostic` does (`spec_decl_name`, WP-172): the qualified
template name, then the canonical arguments with every type scoped,
never the typedef they were written with (`'std::v<std::D<int, int>>'`
for `std::v<_Del>`; `'std::Ptr<int *, std::D<int, int>,
std::alloc<int>>::g'` for a member of a specialization spelled through
the typedefs `_Del`, `_AllocT`). A variable template specialization in
sema's diagnostics (`must be initialized by a constant expression`,
`declaration of reference variable … requires an initializer`, the
`[[deprecated]]` names) prints the same way, with trailing defaulted
arguments dropped (`Sema::name_for_diagnostic`; clang
`VarTemplateSpecializationDecl::getNameForDiagnostic`), even for an
explicit specialization written with a typedef (`template<> constexpr
int g<L *>;` is `'g<long *>'`). `declaration of variable 'au' with
deduced type 'auto' requires an initializer` names the template, as in
clang. Dropping a written default argument from a printed name never
substitutes a dependent default whose shape cannot match it
(`typename T::value_type`, `decltype(T::f())`), so naming
`M<S<int>, int>` never instantiates `S<int>` ([temp.inst]/2; libc++'s
`memory_alignment<simd<bool>, bool>`). Test
`temp.inst/default-arg-elision-no-instantiation.cpp`. The "in instantiation of variable template specialization" note
of an error in a specialization's declaration (its type, or a deduced
type's initializer, substituted with it) names the pattern, as clang
does (`'v'`, `'u<T *>'`, `'S::m'`; `var_frame_names_pattern`); the note
of an initializer instantiated on its own names the specialization
(`'w<int>'`), and so does a member template of a class template
specialization (`'X<char>::p<int>'`). Test
`temp.inst/var-template-instantiation-note-name.cpp` (WP-424).
Test `temp.expl.spec/variable-specialization-names.cpp`. The
Oven's notes ("initializer of 'w<A>' is not a constant expression",
"read of non-const variable 'v<int>'") name the specialization the
same way (`kiln_oven/src/notes.rs`). Test
`expr.const/var-template-spec-note-name.cpp`.
A note on a deleted explicit specialization (of a function template, or
of a member of a class template specialization) points at the
specialization's latest declaration (`Sema::deleted_note_decl`). The
overload candidate note says "has been implicitly deleted" there, since
clang's candidate is not deleted as written; a direct use's note is
"'p<int>' has been explicitly marked deleted here". A member
specialization `template<> void C<int>::q() = delete;` is not a deleted
redeclaration of the instantiated member. Test
`temp.expl.spec/deleted-specialization-notes.cpp`.
At the end of the TU clang instantiates
the definitions a definition requires while that definition's frame is
still open (`PerformPendingInstantiations` inside
`InstantiateFunctionDefinition`), so their notes carry the chain
(`in instantiation of function template specialization 'std::make<int>'
requested here`). kiln's drain does the same: the frame of the pending
definition it just instantiated (captured as it is popped,
`Inst.drain_capture`) is pushed back, untimed, while that definition's
queue and vtable uses drain, and popped when the scope is left (WP-238,
tests/conformance/temp.inst/pending-requester-context.cpp). A definition
queued before the end of the TU is drained with no outer context, as in
clang.

A conversion function template specialization keeps the pattern's
declaration name (`operator T`, which the mangling uses: `cvT_`), but
every printed name of it (notes, `%0` above, constant-evaluation call
notes, the AST dump) uses its target type substituted before any
return type deduction, as clang's instantiated declaration name does:
`'G::operator int<int>'`, `'operator auto (*)(int)<int>'` for a generic
lambda (`ColdDeclData::conversion_spec_target`, read by
`AstNames::write_name`; WP-170).

A class member access that names a conversion function explicitly
(`e.operator Matx<double,3,3>()`, `this->X::operator T*()`) and finds no
member of that name deduces each conversion function template of the
object's class against the named type ([temp.mem]/6, [temp.deduct.conv])
and takes the specializations that deduce as the lookup result; the
bases' conversion templates are not searched, as clang 19's
`LookupDirect` (`Sema::conversion_template_specs`, WP-803,
`tests/conformance/temp.mem/conversion-template-named-explicitly-run.cpp`).

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
- kiln keeps the implicit-definition contexts (clang's "in implicit
  default constructor for …", "in evaluation of exception specification
  for … needed here": `Sema.synth`, `SynthContext`) apart from this
  stack, with their own once-per-depth mark (`synth_noted`). clang has one
  stack, so `Sema::emit` interleaves the two: each `SynthContext` records
  the instantiation depth it was pushed at (`inst_depth`), and the
  instantiation notes of frames pushed after the innermost one print
  before the synthesis notes, the older ones after them (a default member
  initializer instantiated while an exception specification is evaluated:
  the initializer's note, then the specification's; WP-238,
  tests/conformance/class.mem/nsdmi-lazy-context-notes.cpp).

**Deduplication.** `Sema::emit` sets the diagnostic's dedup context to
`InstantiationStack::context_key()`, a hash of every frame's `(kind,
entity, args, poi)` (0 only for an empty stack). The same error in two
instantiations (`S<int>`, `S<long>`) prints twice, each with its notes;
see spec/diag.md §7.4 for the full rule (WP-087). While a class
instantiation substitutes one pattern member, its `DeclId` is mixed into
the key too (`InstantiationStack::member_salt`): clang substitutes each
declarator of `typedef typename T::type g, (*h)(typename T::type);` (or
`T::type a, b;`) separately, so an error in the shared decl-specifier
prints once per declarator, the context notes only with the first
(tests/conformance/temp.inst/shared-decl-specifier-per-declarator.cpp).

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
- `NonDeducedMismatch` names the mismatched part of the parameter type as
  written, at any depth through references, pointers and array elements
  (`'S<T>'` for `S<T> (&)[3]`), never the canonical `type-parameter-0-0`.
  For a reference parameter, P's top-level cv is intersected with A's
  first, an array's cv being its element's (clang's
  `TDF_ParamWithReferenceType`): `const S<T>&` against `const char[3]` is
  `'const S<T>'`, `volatile S<T>&` is `'S<T>'` (WP-152). The same rule
  makes `const T&` from `const char[3]` deduce `T = char[3]`.
- `NonDeducedMismatch` names the argument's template as clang does
  (`written_template_in`): the specialized template, qualified by the
  outermost written qualifier on the way down (an elaborated type's,
  kept through typedefs and aliases, or a deduced placeholder's own
  qualified name). `P{}` for `using P = s::pr<int, long>` prints `pr`,
  `t::Q{}` prints `t::pr`, `s::pr{1, 2}` prints `s::pr`. An
  `initializer_list<E>` parameter whose element cannot be deduced
  reports the element type as written (`'pr<const K, T>'`, also through
  a member typedef in a CTAD guide).
- The `[with …]` bindings of `ConstraintsNotSatisfied`,
  `SubstitutionFailure` and `DeducedMismatch` (and the overload
  ambiguity notes) come from `deduced_args_desc`: a deduced type keeps
  the call argument's sugar (`resugar_call_deduction`: `N::H<int>` when
  written qualified, `H<int>` after a using-declaration); a canonical
  class or enum, deduced through a nested template-id or a base class,
  prints fully scoped (`_Container = std::deque<int>`). Explicit
  template arguments and an argument written as a bare class name are
  wrapped in an unqualified `Elaborated` (`written_bare`) so they stay
  bare. An unnamed template parameter prints as `$N` (clang's
  `getTemplateArgumentBindingsText`).
  Gap: a binding taken from a default template argument
  (`class U = V<T>`) prints its replacement canonical (`U = V<N::P>`
  where clang prints `V<P>`), because `subst` canonicalizes a
  non-dependent replacement.
- An `Inconsistent` type note prints each type with its `aka`
  (`'s3::al<PC>' (aka 'al<pr<const int, long>>')`).
- Tests `temp.deduct.call/deduced-binding-sugar.cpp`,
  `temp.deduct.type/mismatch-template-names.cpp`.
- "constraints not satisfied for alias/class/variable template 'X'"
  names the template bare, however it was qualified
  (`temp.names/constraint-template-name.cpp`).

### 10.5 Other template diagnostics (texts from clang 19; goldens are authoritative)

- "too few template arguments for class template 'X'" / "too many …";
  the note is "template is declared here".
- "template argument for template type parameter must be a type".
- Template-argument checking (`check_template_args`) stops at the first
  argument that fails, as clang's `CheckTemplateArgumentList`: later
  arguments and the arity are not reported (`M<1, int, 2>` is one error).
  Test: `temp.arg.general/first-failing-argument.cpp`.
- A class or alias template named without a template argument list as
  a nested-name-specifier component (`N::Y::v`, `X::type g;`;
  [temp.names]/1) is "use of class template 'N::Y' requires template
  arguments" at the template name, printed with the specifier as written
  (`'::N::Y'`, `'P<int>::J'`; `act_on_nested_name_written`), with
  "template is declared here" at the template (an alias template's at
  its `using`, `conflict_loc`), as clang's
  `diagnoseMissingTemplateArguments` (WP-430,
  `temp.names/missing-args-nested-name-note.cpp`).
- A variable template or concept named alone as a template-argument
  ([temp.names]/1): at compat 22 the parser takes it as a template name
  (P2841, `at_template_name_arg`) and the check reports it against the
  parameter: for a type parameter "use of variable template 'N::v'
  requires template arguments" (qualifier as written, at the name) with
  "template is declared here"; for a template template parameter
  "template argument does not refer to a class or alias template, or
  template template parameter" with "template argument refers to a
  variable template 'v', here"; for a non-type parameter "... must be an
  expression". At compat 19 it is an expression: "use of variable template
  'v' requires template arguments", or for a concept "too few template
  arguments for concept 'C'" (`act_bare_concept_template_arg`). Either
  way, an argument invalid as an expression ends the
  template-argument-list (clang's `ParseTemplateArgumentList`). A concept
  in an expression is "use of concept 'C' requires template arguments".
  Tests: `temp.arg.general/var-template-or-concept-argument{,-clang19}.cpp`.
- A class, variable or alias template-id or a concept-id with dependent
  arguments is checked at its definition (`check_dependent_template_args`,
  clang's `CheckTemplateArgumentList` on dependent arguments; WP-435): the
  arity, a type for a type parameter, a template for a template template
  parameter (a template template parameter argument is matched against
  it, [temp.arg.template]/3), and not a type or template for a non-type
  parameter. A non-dependent non-type argument is converted as for a
  non-dependent template-id (`B<T, 1>` for `int *P` is an error). A
  value-dependent one of known type is checked only for a conversion no
  value can make (clang's `CheckConvertedConstantExpression` on a
  dependent argument, `check_value_dependent_nttp_arg`): an integral or
  `bool` source (not an enumeration) for a pointer, member pointer or
  `nullptr_t` parameter, a pointer for `bool`, and a floating
  conversion either way are "conversion from 'const int' to 'int *' is
  not allowed in a converted constant expression"; a source with no
  implicit conversion sequence is "value of type 'const E' is not
  implicitly convertible to 'int *'" (the source type keeps its cv); both
  at the argument, with "template parameter is declared here" at
  `-fclang-compat=22` only. Narrowing waits for the value, and a
  parameter of class or placeholder type, a parameter pack's type, a
  type that names dependent prior arguments, and an explicit argument
  of a function template-id (`f<i>()`) wait for substitution
  (`nttp_type_pending`). A missing argument needs a default, which is
  substituted with the (dependent) prior arguments and checked as at a
  non-dependent template-id, under "while checking a default template
  argument used here" at the `>`. A template template parameter `TT`
  for a type parameter is "use of template template parameter 'TT'
  requires template arguments". A `T::x` for a type parameter is
  "template argument for template type parameter must be a type; did
  you forget 'typename'?" and the check goes on (clang recovers), so
  `P<T::x, T::y>` reports both; any other failure ends it. Only the
  arguments before a pack expansion are checked; it is "too many
  template arguments" only when the parameters end before it (`P2<int,
  int, Ts...>`), and for an alias template or a concept a pack expansion
  for a parameter that is not a pack is "pack expansion used as argument
  for non-pack parameter of alias template" / "of concept" at the
  expansion, noting the parameter ([temp.alias]/2, CWG1430); both
  only as written, not in a template-id substitution forms on the way
  to re-forming an expansion (`remove_const_t<Ts...>` for an element
  `Ts...` of a fold's pack). A
  template-id naming an expanded parameter pack is checked only for that
  pack's arity (§3.2). Tests: `temp.arg.general/dependent-template-id-checked.cpp`,
  `temp.arg.general/dependent-nontype-conversion{,-19}.cpp`,
  `temp.arg.general/dependent-template-id-converted{,-19}.cpp`,
  `temp.arg.general/dependent-template-id-pack-expansion.cpp`,
  `temp.arg.template/dependent-ttp-argument{,-clang22}.cpp`.
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
pub fn act_on_template_param_end(&mut self, p: DeclId); // after the default, if any: declares p's name ([basic.scope.pdecl]/9)
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
- Each class template redeclaration is its own `ClassTemplateDecl`
  (with its own parameter list and pattern record) linked into the
  chain, as clang's (WP-311). `merge_template_defaults` gives every
  redeclaration's parameters the defaults of the others in
  `Sema::inherited_defaults`, which instantiation reads. That is the
  union over all declarations, so it also reaches earlier ones.
  `check_template_param_list` takes the location of a repeated default
  from whichever earlier redeclaration gave it.
  `tests/conformance/temp.class/redeclaration-diags.cpp`,
  `tests/conformance/temp.class/redeclaration-ast.cpp`.
- A template template parameter's inner list is not order-checked.
- A template template parameter's own default template arguments
  ([temp.param]/14, [temp.arg.template]) complete `TT<args>` when the
  parameter is substituted (`Substituter::ttp_default_args`): each
  missing argument is the parameter's default, substituted with the
  enclosing levels and the arguments before it (`class = T*` names an
  earlier parameter of `TT`'s list, `class = U` an enclosing template's).
  The argument template's own defaults are not consulted for those
  positions. Not done when `TT`'s list has a pack or an argument is a
  pack expansion.
  `tests/conformance/temp.param/template-template-param-own-defaults.cpp`.
- The arity note (`template is declared here`) names the latest
  declaration, as name lookup finds it (`diag_template_arity`;
  `tests/conformance/temp.arg.general/arity-note-latest-redeclaration.cpp`).
- The injected-class-name type `X<P...>` names the parameters of the
  first declaration, even inside a definition that renamed them (clang
  caches it in the chain's common data); a first declaration made by a
  friend declaration defers to the definition's.
- `TT<args>` for a template template parameter `TT` checks only the arity
  against `TT`'s own list (`check_ttp_arity`: "too few/many template
  arguments for template template parameter 'TT'", note at `TT`); a pack
  parameter or a pack-expansion argument skips it, and argument kinds are
  not checked.
- Template template argument matching ([temp.arg.template]/3-4, P0522;
  `templ/ttarg.rs`, from `check_template_arg`) approximates clang's
  "at least as specialized" check structurally instead of deducing through
  an invented specialization: P's parameters are walked against A's (a P
  pack fills the remaining A parameters, an A pack absorbs P's, leftover A
  parameters need a default or must be packs); kinds must match; a
  non-type parameter fits when either type is dependent or contains an
  undeduced `auto`, or both are the same or both integral; a template
  template parameter recurses with the roles swapped. On failure clang's
  `TemplateParameterListsAreEqual(A, P, TemplateTemplateArgumentMatch)`
  supplies the diagnostics: `template template argument has different
  template parameters than its corresponding template template parameter`
  at the argument with the arity / kind / pack / non-type-type note and
  `template parameter is declared here`
  (`tests/conformance/temp.arg.template/`). A non-type parameter of P
  whose type depends on an earlier parameter of the enclosing template
  (`template<class T, template<T> class P>`) is compared with the prior
  arguments substituted (`prior_levels`), as clang substitutes them into
  P before matching; A's parameters are never substituted, and the roles
  swap with the substitution in the nested recursion
  (`dependent-nttp-type.cpp`). Gap: a mismatch found while instantiating
  a member (`DP<V, IP>` in `Outer<int>`) is reported at the template-id,
  not at the argument, since substituted types keep no argument
  locations.
- At `-fclang-compat=22` (WP-394, `templ/ttarg22.rs`; referred to as
  §3.2.1 in the code) matching follows clang 22's
  `isTemplateTemplateParameterAtLeastAsSpecializedAs` (CWG2398, P2113)
  instead: under a partial-ordering context whose diagnostics carry
  "template template argument has different template parameters than
  its corresponding template template parameter" at the argument and
  "previous template template parameter is here" at `P`, `P`'s
  parameters, written as arguments at their own locations (a pack as an
  expansion), are checked against `A`'s parameter list as for a
  template-id `A<...>` (kind, converted-constant conversion, arity; a
  trailing pack expansion left over may be empty, so
  `template<class, class...> class` takes `template<class> class`,
  WP-669 `temp.arg.template/p3-trailing-pack-may-be-empty.cpp`); then
  `A`'s parameters are deduced from them (a conflicting deduction is an
  error); last a constrained `P` must be at least as constrained as `A`
  (subsumption over `CanonTemplateTypeParm` atoms). The substituted
  prior arguments (`Priors`) apply as at level 19
  (`temp.arg.template/*-clang22.cpp`).
- P0846 ([temp.names]/3, WP-336): an unqualified name that ordinary
  lookup finds nothing (no builtin) or only non-member functions for,
  followed by a template-argument-list closed by `>(`, is assumed a
  template-name (`Sema::is_assumed_template_name`). It becomes an ADL
  `UnresolvedLookupExpr` carrying the explicit arguments
  (`act_on_assumed_template_id`); substitution keeps the arguments when
  the definition-context set is empty, so ADL at instantiation deduces
  with them (`get<0>(c)` for `std::complex`). Without the `>(` lookahead
  `<` stays less-than (clang also assumes a template there and
  diagnoses).
- Operator-function-id template-ids ([temp.names]/1,3, WP-364): after
  `operator@`, unqualified or behind a non-dependent qualifier, `<`
  starts a template argument list when `Sema::operator_names_template`
  says the name is a template-name: lookup finds a function template,
  or, unqualified, nothing or only non-member functions (P0846, without
  the `>(` lookahead, as clang); after `template` always; behind a
  dependent qualifier never (`Q<T>::template operator()<char>` becomes
  a `DependentScopeDeclRefExpr` with the arguments). Otherwise `<` is
  less-than. `Sema::act_on_operator_template_id` builds the
  `UnresolvedLookupExpr` with the explicit arguments, completed by the
  call or, without one, by `resolve_single_template_id`
  (`&S::operator()<int>`); unqualified and called with only non-member
  functions found it is an ADL lookup (`operator+<7>(a, b)`), uncalled
  with nothing found it is `use of undeclared 'operator-'`, and a name
  that finds no function template is `no template named 'operator()'`
  (`temp.names/operator-template-id{,-errors}.cpp`). The member-access
  form `x.template operator()<N>()` goes through the member path.
- From C++20 lookup finding only member functions also makes an
  unqualified name or operator-function-id followed by `<` an assumed
  template-name (clang 22; `Sema::is_assumable_function`). A template-id
  whose lookup and argument-dependent lookup find no function template
  is diagnosed by `Sema::diagnose_args_for_non_template`: lexically
  inside a class (a member function body, also through a lambda or an
  out-of-line definition, a default member initializer; the walk skips
  functions and closure types) with a non-empty lookup result "'f' does
  not refer to a template" plus "declared as a non-template here" at the
  first declaration found, elsewhere as an undeclared name. A class member
  found suppresses ADL ([basic.lookup.argdep]/3), so a members-only set
  is diagnosed when the template-id is formed. clang 19 says "member 'f'
  used before its declaration"; C++17's "does not name a template but is
  followed by template arguments" is not implemented (`<` stays
  less-than there). Tests: `temp.names/args-for-non-template-member.cpp`,
  `temp.names/operator-call-template-member-run.cpp` (WP-374).
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
`kiln++ -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0` on
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
- ~~Default member initializers are instantiated with the class~~
  *Resolved (WP-223):* `Substituter::class_body_with` records each
  field's pattern initializer in `inst.deferred_nsdmis` (the class's
  arguments and context pairs in `inst.nsdmi_ctx`) and marks the field
  as having one (`special.nsdmi`). `Sema::instantiate_nsdmi` instantiates
  it at its first use ([temp.inst]/3, clang
  `InstantiateInClassInitializer`): `default_member_init` (a constructor
  or an aggregate initialization) and the implicit default constructor's
  exception specification. It pushes the
  `DefaultMemberInitializerInstantiation` frame; a failure makes the field
  invalid, so later uses are quiet. A local class's or an anonymous
  member's initializers are still instantiated with the class. Tests:
  `temp.inst/lazy-default-member-init{,-errors}.cpp`.
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

Q3 was measured in WP-106 and falls below the bar; see §14.

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
  A conversion function considered as a candidate is treated the same
  way before its return type is read (`add_conversion_candidate`, as
  clang's `AddConversionCandidate`): `operator auto` of a class
  template specialization is instantiated, and one that stays undeduced
  is no candidate (WP-789,
  `class.conv.fct/operator-auto-of-class-template-candidate-run.cpp`).
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
    `sema_decltype_auto_*`, `sema_auto_not_allowed` (then
    `sema_auto_not_allowed_field`; WP-314 merged it with the prototype
    error into one id with clang's position selector),
    `sema_auto_fn_virtual`;
  - `sema_call_incomplete_return`,
    `sema_call_function_incomplete_return`, `sema_cast_to_incomplete`,
    `sema_incomplete_type_required`;
  - `parse_decltype_auto_invalid`.

*Stubs* (each reports `sema_unimplemented_template_feature`, and
`__has_builtin` answers false):
| Stub | Owner |
|---|---|
| `__builtin_operator_new` / `__builtin_operator_delete` in the Oven (sema and codegen landed in WP-088) | WP-061b |

The other §9.5 functions, the source-location builtins and
`__builtin_bit_cast` in the Oven landed in WP-088 (§9.5). Every trait
row is implemented (the overload-resolution traits in WP-130, WP-131
and WP-155; the last staged-off rows in WP-166), so `__has_builtin`
answers yes for every table name.

*Gaps* (each reports a diagnostic or is a known divergence):
- Substituted types carry no argument locations, so the columns of
  some trait errors inside instantiations differ from clang.
- An array of an incomplete type is rejected when the declarator is
  formed, before a trait can see it.
- Abominable function types (`void() const`) are not diagnosed as
  trait operands.
- `ExcSpec::Deferred` is not resolved. `dtor_nothrow` computes the
  implicit destructor's exception specification itself
  ([except.spec]/8): from each subobject's selected destructor whatever
  its access (a protected base destructor leaves the derived class
  nothrow-destructible, libc++'s `std::list` over `__list_imp`); a
  deleted or ambiguous subobject destructor does not make it throwing
  (tests/conformance/except.spec/implicit-dtor-protected-base.cpp).
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
- (Fixed by WP-116.) `decltype(param)` in a dependent return type used
  to profile the `DeclRefExpr` by declaration. Parameters now profile by
  `(index, pack, type)` as in clang (spec/types.md §7.2), so the
  redeclarations match
  (`templates/equiv/temp.over.link-fn-redecl-decltype.cpp`).
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
  An instantiation that rebuilds a call of a custom-checked builtin
  calls the builtin's name again (the `BuiltinFnToFnPtr` cast is
  stripped, as the `FunctionToPointerDecay` of a function's name is), so
  it goes through `builtin_custom_call` once more: `__builtin_constant_p(s)`
  of a non-trivial class object `s` in a template is no
  `-Wnon-pod-varargs` error (WP-683).
- Landed (WP-153): `__builtin_isfpclass(x, mask)` (custom: `x` a real
  floating type, else `floating point classification requires argument
  of floating point type`; `mask` an integer constant expression in
  [0, 1023], else `argument to '__builtin_isfpclass' must be a constant
  integer` / `argument value N is outside the valid range [0, 1023]`;
  result `int`), and `__builtin_expect_with_probability(e, c, p)`, whose
  `p` must be a `double` constant expression (`probability argument to
  __builtin_expect_with_probability must be constant floating-point
  expression`, with the evaluation notes) in [0.0, 1.0] (`probability
  argument to __builtin_expect_with_probability is outside the range
  [0.0, 1.0]`); checked after `convert_call_args`
  (`check_expect_probability`). `-Wassume` landed in WP-160 (spec/sema.md
  §3, `[[assume]]`).
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
- Gaps: `__builtin_source_location` in a function with a deduced return
  type prints `auto` (clang prints the deduced type). (The
  `[T = int]` template-argument suffix inside templates: Landed (WP-140),
  spec/sema.md §7.8.) (`CXXDefaultInitExpr` use locations: Landed (WP-153), see
  **Source location** above.)

**Landed (WP-071): function templates.**
- `deduce_call` (`templ/deduce_call.rs`, §6.1) builds each candidate's
  specialization. Explicit arguments are checked and then substituted
  into P under an `ExplicitArgSubstitution` SFINAE frame, with the
  template's declaration context current (`in_template_context`, as for
  a default template argument, §4.2): `n::h<int>()` of `template <class
  T> void h(decltype(mk<T>()) * = nullptr)` in namespace `n` finds `mk`
  from `n` ([temp.dep.candidate]; WP-387,
  `temp.arg.explicit/substitution-in-template-context.cpp`). Too many of
  them is a `SubstitutionFailure` that carries clang's arity message.
  Before them, as in clang's `DeduceTemplateArguments`, the call's arity
  is checked against the written parameters ([temp.deduct.call]/1):
  fewer arguments than `fn_min_args` is `TooFewArguments`, and more than
  the parameters of a non-variadic template with no function parameter
  pack is `TooManyArguments`, so `f<void>()` of `f(T &)` is "requires 1
  argument" rather than a substitution failure (WP-300). The check after
  substitution stays for expanded non-trailing packs.
  The explicit substitution is clang's `Final` one
  (`SubstituteExplicitTemplateArguments`): each type argument is
  substituted as written (`Substituter::final_subst`, no
  `SubstTemplateTypeParm` sugar), so a parameter it makes
  non-dependent keeps the sugar in its CWG1391 note: `f<s::pl>(1, 2)`
  of `f(W<T>, int)` notes "no known conversion from 'int' to
  'W<s::pl>' (aka 'W<s::plain>')". Every other substitution records
  the canonical argument under `SubstTemplateTypeParm`, which is not
  `aka`-worthy (`print.rs::should_aka`), so a member of `V<s::pl>`
  takes 'W<plain>' with no `aka` (WP-366). Test:
  `tests/conformance/temp.arg.explicit/explicit-arg-sugar-nondependent-param.cpp`.
  Deduced arguments are not substituted as written (WP-373, checked
  against clang 19 and 22): clang 19's `FinishTemplateArgumentDeduction`
  substitutes the canonical deduced list, so `f(x, 1)` of `f(T, typename
  id<W<T>>::type)` with `x` an `s::pl` notes 'typename id<W<plain>>::type'
  (aka 'W<s::plain>'), as kiln does; clang 22 substitutes the deduced list
  `Final` but deduces `T` from the call argument's canonical type, so
  only its printing differs (`id<W<s::plain>>`, the clang-22 scoped
  printing of a bare class, left to the clang-22 switch). The
  specialization's own arguments are canonical in both (clang's
  `TemplateArgumentList` comes from the canonical builder), so a
  candidate note naming it binds them canonically, `[with T = s::plain,
  U = int]` for `f<s::pl>(x, i, 1)` with `i` a `myint`
  (`overload/diag.rs` `spec_desc`), while a failed deduction's
  bindings keep the argument's sugar (`'myint' (aka 'int')`). Test:
  `tests/conformance/temp.deduct.call/specialization-binding-canonical.cpp`.
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

**Landed (WP-106): function templates, remainder.**
- **Explicit specialization and instantiation** (§6.9, `templ/funcs.rs`):
  the parser accepts `template<>` / `[extern] template` before a
  non-class declaration and template-id declarator ids.
  `act_on_function_spec_declarator` matches by [temp.deduct.decl]
  (`deduce_funcaddr`, which substitutes explicit args into the whole
  function type, then `more_specialized`) with clang's no-match /
  ambiguous / after-instantiation errors; an explicit specialization is an
  unregistered redeclaration of the specialization
  (`AstBuilder::function_spec_redecl`). Members of class template
  specializations (`template<> int A<int>::f()`, `template int A<char>::f();`,
  `extern template`) go through `member_of_spec_candidate` and
  `act_on_explicit_member_{spec,inst}` (MemberSpecInfo transitions).
  `template<> template<>` heads, in-class explicit specializations and
  operator template-ids work. A member class specialization
  (`template<> struct O<long>::I { ... };`, WP-172): `act_on_tag` gives
  the `template<>` to the qualifier (it is cleared before the body, so
  the class's own member functions are not function template
  specializations) and marks the member `ExplicitSpecialization`;
  `scope_slots` stops at it, so its out-of-line members are defined
  without `template<>` ([temp.expl.spec]/5). Member *enumeration*
  specializations (`template<> enum class O<long>::E { ... };`) still
  report "not yet implemented".
- **Template candidates**: conversion-function templates
  ([temp.deduct.conv], `deduce_conversion`, `more_specialized_conversion`),
  `operator->` templates (`arrow.rs`), reversed/rewritten operator
  templates ([over.match.oper]/3.4, `add_rewritten_template`,
  `more_specialized_operator`), `&f` deduction notes (`address.rs`), and
  dependent member template-ids (`act_on_template_member`,
  `act_on_member_template_args`). A member template-id with dependent
  arguments is a dependent member access again at instantiation. It keeps
  the access's nested-name-specifier, so `this->Base::g<T>()` looks `g`
  up in `Base` even when the derived class declares a non-template `g`
  (WP-801,
  `tests/conformance/temp.names/p3-qualified-member-template-id-dependent-args-run.cpp`).
  Gap: `(*this).Base::g<T>()` does not parse `<` as a template argument
  list, because the dereferenced `this` is a dependent type.
- **`std::initializer_list`** ([dcl.init.list]/5): `CXXStdInitializerListExpr`
  over a `MaterializeTemporaryExpr 'const E[N]'`; `init.rs`
  `std_initializer_list_init`, `init_list_ctor_init` ([over.match.list]
  phase 1), `reference_list_init`; `auto x = {..}`; Oven and codegen store
  the begin pointer and size (or end pointer).
- **Template parameter objects and reference NTTPs**: §6.10 "As built".
- Gaps, with owners:
  - `template<> template<class U> int A<long>::g(U)` (a member template of
    an implicit specialization redeclared as a template);
  - a declared-but-undefined explicit member specialization can still be
    instantiated from the pattern (one-line hook proposed in `class.rs`,
    WP-109's file);
  - [temp.func.order]/6 (reversed-candidate constraint rules);
  - the expression printer drops explicit template args (`C{}.m<int>(1)`);
  - a specialization whose body failed to instantiate is not marked
    invalid;
  - dependent NTTP types (`template<class T, T V>` with a class `T`) and
    the "while substituting prior template arguments" note;
  - reference/pointer NTTP values with a non-zero offset (subobjects,
    `&arr[1]`) are not substituted back;
  - a base-specifier note with an access keyword points at the keyword,
    not the type (`BaseSpec` has no type location);
  - class-value mangling is the §6.10 seam (WP-079);
  - explicit instantiation definitions are emitted `external`, clang
    uses `weak_odr` + comdat; `extern template` does not suppress
    emission (WP-113);
  - a pointer to an automatic variable at the top of a constant
    expression is accepted by the Oven (`constexpr int* p = &local;`,
    Oven owner).

**Landed (WP-073): concepts.**
- State: `InstState::concepts: ConceptState` (`templ/concepts/mod.rs`)
  holds `sats: Vec<Satisfaction>` (a `SatId` indexes it), `sat_cache:
  (first(concept or entity), canonical args) → SatId`, `expr_sat:
  ExprId → SatId` (the satisfaction behind a non-dependent
  `ConceptSpecializationExpr`/`RequiresExpr`, for notes), the recursion
  guard `checking`, `norm: NormTable` with `normal_forms` and
  `subsume_cache`, `placeholder_locs` (the type-constraint location of
  a `C auto` variable or function), and `counters: SatCounters { count,
  cache_hits, subsume }` (`sat.count`, `sat.cache_hits`).
- Satisfaction (`concepts/sat.rs`) walks the constraint *expression*
  (`&&`, `||`, parens, a bool `ImplicitCast` over them), as clang 19
  does. Each atom is substituted under a `ConstraintSubstitution`
  SFINAE frame, must have type `bool`
  (`sema_non_bool_atomic_constraint`), and is evaluated by the Oven
  (`EvalKind::Constraint`). A concept-id is checked by `check_concept`
  under a `ConceptCheck` frame ("while checking the satisfaction of
  concept '…' requested here") and memoized. Template constraints use
  `check_constraints` under `ConstraintsCheck`, memoized only for
  single-level argument lists. A non-dependent concept-id or
  requires-expression stores its value in `ExprNode.aux`
  (`ExprArena::satisfied`/`set_satisfied`), and the Oven folds that.
  The §8.3 `atom_cache` landed with WP-109 (below).
- Conservative fallbacks (WP-073; replaced by WP-109's fallback policy,
  below, and §8.3): an atom was treated as *satisfied* when its
  substitution hit `sema_unimplemented_template_feature`, or when it was
  still value-dependent after substitution.
- Call sites:
  - `deduce_call.rs` checks the associated constraints after deduction
    (`Tdk::ConstraintsNotSatisfied`, `DeductionFailure::sat`, and the
    note "candidate template ignored: constraints not satisfied
    [with …]" plus its chain).
  - `ensure_template_constraints` checks class, variable and alias
    template specializations (`get_class_spec`, `get_var_spec`,
    `alias_template_type`; [temp.names]/9). An alias is checked when its
    arguments and template name are not dependent, before the aliased
    type is substituted; inside a requires-expression or deduction the
    error is a substitution failure, so `requires { typename
    std::projected<int, F>; }` is false
    (tests/conformance/expr.prim.req.type/constrained-alias-template.cpp,
    temp.constr.decl/alias-template-unsatisfied.cpp; WP-309).
  - `order.rs::deduce_partial` checks a partial specialization's head
    constraints after matching.
  - `find_partial_decl` tells same-argument partial specializations
    apart by their printed head constraints.
  - `act_on_static_assert` prints the chain for a failing
    concept-id/requires-expression.
- Requires-expressions (`concepts/requires.rs`): `Substituter::
  requires_expr` substitutes the parameters and evaluates requirements
  in order:
  - simple and type requirements must be valid;
  - compound requirements check `noexcept` (`Sema::can_throw`, which
    now covers `CXXMemberCallExpr`) and the return-type-requirement on
    `decltype((E))`;
  - nested requirements are evaluated.

  A non-dependent requires-expression is evaluated in
  `act_requires_expr` and keeps its written requirements in the AST.
  The parser accepts an unqualified type-requirement
  `typename X<T>;`.
- Subsumption (`concepts/normal.rs`, `concepts/subsume.rs`): normal
  forms are hash-consed `NormNode::{Atomic{expr, mapping}, And, Or}`.
  A concept-id is expanded through its arguments, with a recursion
  guard. `subsumes(p, q)` uses DNF(p) against CNF(q), capped at 65,536
  clauses (over the cap counts as "does not subsume"), and is cached.
  `more_specialized`/`partial_more_specialized` apply
  [temp.func.order]/6 when both directions deduce, and an
  unconstrained template is the weakest.
- Constrained placeholders: `check_placeholder_constraint`
  (`templ/auto.rs`) checks `C<deduced, args…>` for `C auto` variables
  and deduced return types. It reports "deduced type 'T' does not
  satisfy 'C<args>'" at the concept name (`DeclSpec::constraint_loc`).
  Invented parameters of abbreviated templates carry their constraint
  at that location too (`act_on_invented_template_param` takes `(C,
  args, loc)`).
- The WP-073 divergences (note locations, non-bool atoms, the
  `static_assert(requires {…})` print, the ambiguous-atom notes) and
  gaps (trailing requires-clauses on members of class templates,
  `atom_cache`) were closed by WP-109 (below). Generic lambdas wait for
  lambdas.
- The `<type_traits>` probe went from 20 errors to 10. What remains:
  - 4 are "not yet implemented: pack expansion" (WP-072), at
    `common_reference.h:182,183`, `conjunction.h:39` and
    `disjunction.h:44`;
  - 2 are "reference to a member of a dependent context" in the
    `__basic_common_reference_t` alias (`common_reference.h:160,162`),
    which is alias instantiation, not concepts;
  - 4 are unrelated to WP-071/073: `__int128_t` and its follow-ons.
**Landed (WP-072): packs.**
- `templ/pack.rs` expands by probing (§7.1): the pattern is substituted
  once with `probing` set; the first pack reached records its length
  (and name) and aborts the probe; the pattern is then substituted once
  per element. A probe that reaches no pack leaves the expansion as
  written (`Expansion::Unexpanded`). Packs of different lengths are
  "pack expansion contains parameter packs 'Ts' and 'Us' that have
  different lengths (2 vs. 3)" at the `...` (clang's `EllipsisLoc`).
- Expansion sites: template argument lists, function parameter types,
  call arguments and paren lists, braced lists (`S{ts...}`), paren
  initializers (`P p(ts...)`, also in the parser), mem-initializer
  expansions (`B(ts)...`) and their argument lists, base-specifier
  packs, type-trait arguments, and function parameter packs
  (`DeclOrPack::Pack`).
- Folds (§7.3) are built iteratively along the spine with `act_binary`
  at the `...`. Empty folds give `true`, `false` or `void()` at the
  `...`; any other operator is an error there. R-WALK:
  `stress/fold-65536.cpp` folds 65,536 elements through the Oven (2
  steps, 1 call, 8 bytes; 0.12 s of Oven wall time in a debug build) and
  codegen, and runs. clang 19 cannot compile it (bracket depth 256;
  raised, it overflows its stack), so it runs with `oracle: none`. The
  dumper is not exercised there: clang's indented format is quadratic
  in the depth (about 21 GB of text for 65,536); a 4,096-deep fold
  dumps fine (85 MB).
- Deduction (§6): a trailing function parameter pack takes the
  remaining call arguments; packs in template argument lists match
  the remaining elements (`deduce_arg_list_packs`); explicit arguments
  form a pack prefix that deduction extends; an undeduced pack is
  empty. Conflicts give clang's note (`deduced conflicting types for
  parameter 'Ts' (<int> vs. <char>)`). Partial specializations of a
  variadic primary store their written arguments as `Pack`s, and
  matching and ordering compare the flattened lists.
- `__make_integer_seq` and `__type_pack_element` take expanded packs.
- Pack indexing (P2662), `T...[i]` and `p...[i]`: parsed after a type
  name in a decl-specifier and after an id-expression. Substitution
  yields the selected element. An index out of range is clang's
  "invalid index 1 for pack ts of size 1" (an expression is named
  unquoted, at the pack's name) or "invalid index 0 for pack 'Ts' of
  size 0" (a type is quoted, at the index).
  Indexing a non-type template parameter pack (`Is...[0]`) works in
  kiln, but clang 19 rejects it ("cannot initialize ... with an lvalue
  of type '<dependent type>'"), so no test pins it.
- Gaps, with owners:
  - `sizeof...` after substitution is an `IntegerLiteral` of `size_t`,
    not a `SizeOfPackExpr` with the length (dump gap; WP-072 follow-up).
  - A substituted `PackIndexingExpr`/`PackIndexingType` is replaced by
    its element; clang keeps the node as sugar (dump gap).
  - Partially substituted pack expansions (`tl<tl<Ts, Us>...>` with
    only `Ts` known) need clang's `SubstTemplateTypeParmPackType`:
    unimplemented ("partially substituted pack expansion").
  - Lambda init-capture packs: §7.1 (WP-292). Fold-expanded constraints: Q17.
  - Pre-existing, seen while testing: `int a[] = {sizeof(Ts)...};`
    reports "sizeof incomplete type" (a reference to an incomplete
    array with a dependent initializer should be type-dependent);
    `__is_same(H::type, int)` with a dependent `H` does not parse;
    `L::member` through an alias `L` of a class template specialization
    is "expected a type"; an `auto` function whose instantiated body
    fails deduces `void`, giving a follow-on error at the caller.
- The `<type_traits>` probe went from 20 errors to 18 on this branch
  (based before WP-073): the pack expansions are gone; the rest are
  constrained partial specializations and requires-expressions
  (WP-073, already on main) and `__int128_t` (WP-107). Main after
  WP-073 has 10, of which 4 are pack expansions that this branch fixes.

**Landed (WP-109): concepts and alias follow-ups.**
- Dependent member templates as template template arguments:
  `X<T>::template M` (and `X<T>::M` naming a member template of a
  dependent specialization) is a `DependentIdentifier` template name,
  looked up again at substitution; inside the current instantiation it
  resolves at once to a `Qualified` name. A template template parameter
  replaced by a `Qualified` name yields the underlying declaration. This
  fixes `__basic_common_reference_t` (`<type_traits>` reaches 0 errors).
  `unqualified()` peels sugar that buries qualifiers. An alias template
  redeclared with the same type is accepted.
- Fallback policy (replaces WP-073's): an atom or requirement whose
  substitution hits `sema_unimplemented_template_feature` is a hard
  error (the captured diagnostic is emitted, nothing is assumed). A
  value-dependent result is *satisfied* only while the levels themselves
  are dependent (a retained level or a dependent argument: a partial
  check during the definition). With concrete levels, a dependent result
  means the substituter left something incomplete, which is a kiln bug. It
  is reported as "not yet implemented" rather than assumed either way, and
  no known program reaches it. libc++'s `sized_sentinel_for` and
  `assignable_from` used to reach it through an ADL call to an abbreviated
  hidden friend. They are now satisfied normally, because that friend is a
  function template ([dcl.fct]/22; WP-307 (1),
  tests/conformance/temp.constr.atomic/abbreviated-friend-template.cpp;
  re-checked by WP-315). Fold-expanded constraints `(C<Ts> && ...)` evaluate
  through the substituted fold.
- `atom_cache` (§8.3): `ConceptState::atom_cache: (atom ExprId,
  Vec<canonical ArgListId>) → (bool, records)`; `sat_atom` wraps
  `sat_atom_uncached`. An atom's `ExprId` belongs to one entity's
  constraint, so the key is that entity's levels; this replaces
  `NormId` in §8.3's key. Not cached when a level is dependent or the
  evaluation emitted or trapped an error. Single-level checks are
  already served by `sat_cache`, so hits come from multi-level
  constraints (member templates of class template specializations).
  `SatCounters` gains `atoms`/`atom_hits`; the instantiation report's
  JSON totals row gains `"sat"`, `"sat_cached"`, `"atoms"`,
  `"atoms_cached"`, and the text format a `satisfaction: N (M cached)
  atoms: A (H cached)` line (§11.3).
- Note locations: a type-requirement's note is at the type
  (`Requirement::Type(QualType, Loc)`), a `noexcept` requirement's at
  the keyword (`Requirement::Compound { noexcept: Option<Loc>, .. }`),
  a member access failing after substitution at the `.`/`->`
  (`TemplateTables::member_op_loc`).
- A non-`bool` atom is diagnosed where it is written
  (`Sema::check_constraint_expression`, called by `act_concept` and the
  requires-clause parser). The concept's constraint (or the clause)
  becomes a `RecoveryExpr`; naming such a concept is an error expression
  with no further diagnostic, and a clause's atom is unsatisfied.
- `static_assert(requires {…})` prints the expression as clang does
  (`AstNames::expr` has a `RequiresExpr` arm).
- After an ambiguous call between non-comparable constrained candidates,
  `ambiguous_constraint_notes` (`concepts/subsume.rs`, called from
  `with_ovl_notes`) adds clang's `note_ambiguous_atomic_constraints` and
  `note_ambiguous_atomic_constraints_similar_expression`.
  `exprs_equivalent` compares `sizeof(T)` and type-trait operands by
  canonical type. (Superseded by WP-116: it compares canonical profiles,
  below.)
- Trailing requires-clauses of members of class templates
  ([over.match.viable]/3, [over.match.best]/2.6). As in clang, the
  instantiated member keeps the pattern's clause, which is checked with
  the specialization's levels:
  - `ConceptState::member_constraints: DeclId → (pattern DeclId,
    MultiLevelArgs)` is filled by `function_decl` (non-template
    members) and `member_template` (member templates, whose
    `associated_constraints` and `check_constraints` then use the
    pattern's clause with the outer levels plus their own), and by
    `friend_function_template` for the namespace-scope template a
    constrained friend template redeclares, so a constraint naming the
    enclosing class's parameter ([temp.friend]/9, `requires C<T>` on
    `template<class U> friend int f(X, U)`) sees that specialization's
    arguments (WP-231);
  - an atom of such a clause is substituted with the pattern classes
    mapped to their instantiations ([`Sema::inst_context_pairs`]) from
    the member's class, or for a friend (whose semantic context is the
    namespace) from its lexical class (`inst_lexical_class`,
    [temp.friend]/1): a hidden friend of a member class or member class
    template of a class template (`view<V>::iterator`, libc++'s
    `filter_view::__iterator`, `join_view::__iterator<Const>`) names
    that member class, a member of the enclosing specialization, in its
    parameter types and constraints ([temp.dep.type]/1), and without the
    mapping `find_instantiated_decl` found no instantiated parent ("not
    yet implemented: reference to a member of a dependent context"),
    including when the friend is found only by argument-dependent lookup
    inside a concept's requires-expression (WP-272);
  - *Audit (WP-278): this matches the standard.* A friend defined in a
    class is in that class's scope ([class.friend]/7), so its
    requires-clause binds names where it is written: the enclosing
    classes' parameters, member typedefs, static members and member
    class templates' parameters. Those names bind in the pattern, so a
    class-scope `test` hides a namespace-scope `test` even though the
    atom is then substituted in the friend's namespace
    (`in_inst_context`, which only re-runs dependent ADL). The
    constraint is not instantiated with the class
    ([temp.inst]/17). Instead each atom is substituted when satisfaction
    is checked ([temp.constr.atomic]/3), using the arguments of the
    specialization that instantiated the friend, i.e. `levels` plus the
    lexical class's `inst_context_pairs`. Every such specialization
    declares a distinct function ([temp.friend]/9 last sentence:
    a constrained friend declares no function of any other scope), so
    the per-specialization mapping is the whole story. No other
    declaration's arguments need merging, and [temp.constr.decl]/3's
    associated constraints are only that clause (a non-template
    friend has no template-head). `inst_lexical_class` falls back to the
    semantic context for members, so a member function keeps its own
    class mapping. Probed against clang 19 for `requires C<T> && B`
    (outer and member-template parameters), a member typedef, a static
    data member, and a class-scope static member template shadowing a
    namespace function.
  - `Sema::check_function_constraints(f, loc) -> Option<SatId>`; a
    non-templated function's clause (already an error, `non-templated
    function cannot have a requires clause`) is still checked, with no
    template arguments, as clang does (`ConceptState::
    untemplated_requires`; WP-294, sema-class.md §4.7.1);
    overload resolution marks a failing candidate
    `CandidateFailure::ConstraintsNotSatisfied { sat }` ("candidate
    function not viable: constraints not satisfied" plus the chain);
  - `order_constrained_candidates` (`overload/best.rs`) orders viable
    non-template candidates with the same parameter-type lists
    (`same_param_type_lists`, [over.match.best]/2.6) by
    `more_constrained`; constructor selection and special-member
    deletion now call `order_template_candidates` too;
  - naming a function whose constraints fail is
    `sema_reference_to_unsatisfied_function` ("invalid reference to
    function 'f': constraints not satisfied",
    `Sema::diagnose_unsatisfied_use`). A member access naming such a
    member (`x.f`, `x.operator->`, the only candidate) is then an error
    expression, as clang's failed `DiagnoseUseOfDecl` is `ExprError`:
    the call never deduces the member's placeholder return type, so its
    definition is not instantiated ([temp.inst]/5), and inside a
    requires-expression the requirement is just unsatisfied (WP-298;
    libc++ `counted_iterator::operator->` probed by `iterator_traits`;
    `tests/conformance/over.match.viable/unsatisfied-member-auto-return.cpp`);
  - special-member redeclaration matching also compares the trailing
    requires-clauses, so constrained copy constructors overload;
  - friends attach their trailing requires-clause; hidden friends
    defined in class templates are no longer replayed as members
    (previously "not yet implemented: Function members of class
    templates").
- AST dump: a non-dependent `RequiresExpr` prints ` satisfied` or
  ` unsatisfied` (spec/ast-dump.md); a `MemberExpr` built in an
  unevaluated operand is `non_odr_use_unevaluated`; an unqualified
  type-requirement keeps clang's `ElaboratedType 'typename X'`. The
  xtask clang-ast normalizer accepts non-dependent requires-expressions
  (a requirement's own `satisfied` is dropped: kiln records only the
  expression's value) and `MemberExpr.nonOdrUseReason`.
- Known divergences from clang:
  - a non-`bool` atom: clang drops the whole declaration (later uses
    are "use of undeclared identifier"), kiln keeps it;
  - a fold-expanded constraint's note is "'... && ...' evaluated to
    false", where clang expands the fold per element ("because 'long'
    does not satisfy 'Int'");
  - "no matching function" for a qualified call of a function template
    is at the name, not the qualifier (deduction path);
  - an instantiated requires-expression keeps no requirements in the
    dump (its value and notes are kept).
- Gaps, with owners:
  - eligibility and triviality of special members by their constraints
    ([special]/6): unowned;
  - conversion functions and user-defined conversion sequences
    (`ics.rs`) do not order constrained candidates: unowned;
  - `__is_convertible` landed in WP-131, and
    `temp.constr.concept-std-like.cpp` passes again.

**Landed (WP-116): expression equivalence ([temp.over.link]).**
- `templ/equiv.rs` is the equivalence API on `Sema`:
  - `exprs_equivalent(a, b) -> bool` and `opt_exprs_equivalent` compare
    canonical expression profiles (spec/types.md §7.2, "Landed
    (WP-116)"). Both moved here from `overload/redecl.rs`. The old
    structural walk and its `depth` parameter are gone, and the callers
    in `concepts/subsume.rs` were updated.
  - `pub fn equivalent_signatures(&mut self, a: DeclId, b: DeclId) ->
    bool`: true when two function declarations or function templates
    (each a `FunctionTemplate` or its pattern) declare the same entity
    under [temp.over.link]/7. That means equivalent template heads,
    including requires-clauses, plus the same declared type and the same
    trailing requires-clause. It is `fn_corresponds` with `b` as the new
    declaration. WP-106 and later ordering/redeclaration code should
    call this instead of comparing printed or structural forms.
  - `check_template_heads_match(new, old) -> bool` checks a class,
    variable or alias template redeclaration's head against the
    template's, with clang's `TemplateParameterListsAreEqual`
    (`TPL_TemplateMatch`) checks and locations. In order:
    - arity (`sema_template_param_list_different_arity` at each list's
      `template` keyword);
    - kind (`sema_template_param_different_kind`);
    - pack (`sema_template_parameter_pack_non_pack` /
      `note_template_parameter_pack_here`);
    - non-type parameter type (`sema_template_nontype_parm_different_type`
      / `note_template_nontype_parm_prev_declaration`);
    - nested template template heads (`which = 1`);
    - type-constraint (`sema_template_different_type_constraint`);
    - requires-clause (`sema_template_different_requires_clause`);
    - each followed by `note_template_prev_declaration`.

    On a mismatch the redeclaration is not chained. That covers
    `act_on_typedef` for an alias template,
    `finish_template_pattern` for a class or variable template, and the
    `act_on_tag` declaration path through `check_redeclared_class_head`.
    Function templates with different heads are overloads, so they
    never reach this check.
  - A friend class template declared in a dependent context
    (`template<C U> friend class X;` in a class template,
    `is_dependent_friend_template`) has a head deeper than `X`'s that
    may name the enclosing parameters. As in clang's
    `CheckClassTemplate`, it is compared with neither side when it is
    declared; other redeclarations compare with the latest declaration
    before it that is not such a friend (`head_comparable_decl`). When
    the enclosing class is instantiated, `check_friend_class_head`
    (`templ/subst/decls.rs`) substitutes the friend's head, including
    type-constraints and requires-clause, in a SFINAE frame and checks
    it against that declaration, under the instantiation note (clang's
    `TemplateDeclInstantiator::VisitClassTemplateDecl`; WP-167, tests
    `temp.friend/friend-template-constrained-head.cpp` and
    `friend-template-head-differs.cpp`). Deviation: clang compares
    with the latest instantiated friend, so with two instantiated
    dependent friends of one template it can diagnose (and note) where
    kiln does not.
- Canonical dependent types and dependent template arguments use real
  profiles. Equivalent alias template redeclarations therefore chain,
  e.g. libc++ `__swap_result_t` in `__utility/swap.h:33`. Function
  template redeclarations whose `decltype`, `noexcept` or array-bound
  expressions are equivalent also match. Class and variable partial
  specializations with equivalent arguments are the same specialization.
- `concepts/sat.rs` `find_partial_decl` compares template heads by
  pairwise expression equivalence (`head_signature` returns `ExprId`s, no
  longer printed strings).
- Typedef redefinition follows clang's `isIncompatibleTypedef`:
  - when either type is dependent, the check is skipped and a mismatch
    is diagnosed at instantiation. Alias templates are always compared;
  - the kind in `sema_redefinition_different_typedef` is the previous
    declaration's (typedef, type alias, or alias template);
  - in class scope, redeclaring a typedef-name with the same type is
    `sema_redefinition` ([dcl.typedef]/4, CWG56/424);
  - an invalid typedef member of a class template is not instantiated
    (`templ/class.rs`).
- `expr.rs` `builtin_unary`: `*e` on a type-dependent operand that is not
  of an overloadable type (`T (&a)[N]`) builds a dependent unary
  expression, as clang's `CheckIndirectionOperand` does, instead of
  "indirection requires pointer operand".
- Explicit specializations of function templates are still unimplemented
  (`act_on_specialization_declarator_id`). When they land, clang matches
  them by deduction plus `hasSameType`, and canonical types are now
  profile-based, so no extra equivalence step is needed.
- `templates/equiv/temp.over.link-head-locations.cpp` passes since
  WP-121: an unnamed template parameter is located where its name would
  be (the token after its key, `...` or declarator; clang's `NameLoc`),
  and a type parameter's key is recorded as its begin location
  (`ColdDeclData::begin_loc`).
- Not done here (BACKLOG WP-116 extras): `sat_fold` per-element notes;
  dropping a declaration with a non-`bool` atom; keeping requirements in
  an instantiated `RequiresExpr`.
- Tests: `tests/conformance/templates/equiv/` (27).

**Landed (WP-121): dependent NNS chains, friend templates, trailing
requires-clause scope.**
- *Dependent nested-name-specifiers* ([temp.dep.type]/6, [temp.res]):
  an NNS that is still dependent after substitution is rebuilt
  (`nns_still_dependent`); `find_instantiated_decl` keeps a member of a
  parent that depends only on retained levels (`ctx_template_levels`);
  the missing `typename` / `template` diagnostics name the canonical
  context. A builtin template-id with a dependent argument
  (`__make_integer_seq<S, T, N>::`) is a dependent context
  (`is_dependent_ctx` includes `BuiltinTemplate`). Unnamed NTTP packs
  are supported.
- *Friend templates* ([temp.friend]): `template<class U> friend class X;`
  (`act_on_friend_class_template`, `Sema.friend_class_template`),
  a qualified `template<class U> friend class N::X;` /
  `template<bool> friend class A<T>::B;` befriending the class template
  qualified lookup finds, without declaring anything
  (`act_on_qualified_friend_class_template`; a member template is found
  again in the instantiated class; another kind of entity is
  `sema_redefinition_different_kind`, nothing found in a namespace
  `sema_friend_def_no_match`); an unqualified name that ordinary
  lookup finds as a member class template of an enclosing class (the
  search stops at the innermost enclosing namespace, [namespace.memdef]/3,
  so class scopes are searched) is befriended the same way, as if
  qualified (WP-278; libc++ `elements_view::__iterator`'s
  `template <bool> friend class __sentinel;`;
  `tests/conformance/temp.friend/friend-member-class-template-of-enclosing-class.cpp`),
  `template<class U> friend R f(U);` (`friend_function_template`), and
  friend specializations `friend R f<>(...)` / `friend R N::g<T>(...)`
  (`Declarator::friend`, `friend_spec_template`,
  `act_on_friend_spec_without_template`, `function_templates_in`,
  `match_function_spec_in`). A dependent friend specialization is a
  `FriendSpec` in `Sema.friend_specs`, matched when its class is
  instantiated (`instantiate_friend_spec`); `friend_template_of` finds
  the befriended template for access checks. Diagnostics:
  `sema_dependent_friend_spec_no_candidate` with
  `note_dependent_spec_not_template`.
- *Template redeclarations* share one identity: `Ast::first`/`latest`
  resolve a template through its pattern's redeclaration group. A friend
  class template naming an existing template is a redeclaration in that
  group and does not make the original dependent. A class template
  redeclared as a non-template class, a variable, a function, or a
  variable, function or alias template (or the reverse) is "redefinition
  of 'X' as different kind of symbol" with "previous definition is
  here"; `template<> struct NT;` for a non-template class `NT` is
  "extraneous 'template<>'" plus clang's "specialization of member" error
  (WP-409, `temp.pre/class-template-redeclaration-kind.cpp`).
- *Inherited default arguments* ([temp.param]/12):
  `Sema.inherited_defaults`, filled by `merge_template_defaults`. The
  AST records, separately, what clang records:
  `record_inherited_defaults` gives each parameter of a redeclaration
  that has no default of its own the parameter owning its counterpart's
  default in the previous declaration (`TemplateTables::
  inherited_defaults`, clang `setInheritedDefaultArgument`). It runs
  only when the heads match. The dump prints it as `inherited from`
  (spec/ast-dump.md). Deduction completes a dependent `P` with its
  defaulted arguments (`complete_dependent_args`, [temp.deduct.type]).
  The parameters and defaults, and the defaults' depth, are the head
  declaration's (`head_template`, §6.2; WP-286, WP-295, WP-296).
- *Trailing requires-clause* ([basic.scope.param]/1, [expr.prim.this]/2):
  the function parameters (and `this` in a member) are in scope.
  Satisfaction substitutes the pattern's parameters into fresh ones
  (`Substituter::seed_function_params`, clang
  `addInstantiatedParametersToScope`), sets the specialization's `this`
  type, and maps pattern contexts to the specialization's for members
  of class templates (`constraint_function_scope`,
  `inst_context_pairs`).
- *Out-of-line members of class templates*: a later out-of-line
  initializer that names another out-of-line-defined static member
  finds that member's redeclaration; `find_instantiated_decl` maps it
  through its first declaration.
- *Template parameter locations*: an unnamed parameter is located where
  its name would be (`act_on_type_template_param`,
  `act_on_constrained_template_param` and `act_on_template_template_param`
  take that location); a type parameter's key is its begin location.
- Gaps: friend templates declared outside a class; an owned friend class
  in a class template; the dependent friend specialization placeholder
  in dumps; codegen of out-of-line class template member definitions
  (pre-existing panic in `kiln_abi/src/irtype.rs`).
- Tests: `tests/conformance/temp.res/`, `temp.friend/`, `temp.param/`,
  `basic.scope.param/`, `dcl.meaning/qualified-declarator-scope-template.cpp`,
  `templates/deduce/temp.deduct.type-defaulted-args.cpp`, and the
  `ast/` goldens for each.

**Landed (WP-155): out-of-line member classes, partial-specialization
declarator scopes.**
- *Out-of-line member classes and enums* ([temp.mem.class],
  [temp.mem.enum]): `template<class T> struct A<T>::B { ... };` defines
  the member class declared in the class template; it is a pattern for
  `MemberOfSpecialization` (§4.6) and is instantiated when completeness
  is needed. Members of a member class of a partial specialization, and
  member class templates, are supported (`templ/mod.rs`, `decl.rs`
  `act_on_tag`; the parser's `parse_tag_qualifier`).
- *Constructors of partial specializations*: `X<T*>::X(` is a
  constructor declarator. The parser no longer annotates the template
  scope ahead (`annotate_template_scope`) when `at_special_declarator`
  holds.
- *Non-type arguments in a declarator scope*: a dependent template-id
  keeps its written arguments (§3.4), so `template<int S> void
  B<1, S>::g()` did not find the partial specialization `B<1, S>`
  (registered with converted arguments). When entering a declarator
  context, `entered_partial_by_converted_args` converts the arguments
  with `check_template_args` and looks again (`find_partial`). It does
  so too when the qualifier leaves arguments to the primary template's
  defaults (`E<W<U>>::I` for `E<T, S = typename T::S>`), whose
  converted form the partial was registered with (WP-709). Gap:
  dependent template-ids elsewhere are still neither checked nor
  converted (clang checks every template-id).
- *Template arguments of a declarator qualifier*: only the qualifier's
  own components are entered. Its template arguments are parsed with
  the entering context off (`parse_template_arg_list`, as clang's
  `EnteringContext = false`), so in `template<class T> int F<T,
  typename X<T>::type>::g()` the inner `X<T>` is an ordinary dependent
  specialization rather than the current instantiation of `X`; the
  argument stays `typename X<T>::type` and matches the partial
  specialization ([temp.dep.type]/1; WP-726, fmt 10's `formatter`).
- Tests: `tests/conformance/temp.mem.class/`, `temp.mem.enum/`,
  `temp.spec.partial.member/out-of-line-ctors.cpp`,
  `temp.spec.partial.member/non-type-arg-scope.cpp`,
  `temp.spec.partial.member/out-of-line-member-defaulted-args.cpp`,
  `templates/builtins/builtin.is-trivially-constructible-assignable.cpp`.

**Landed (WP-176): libc++ `<type_traits>` follow-ups.**
- *Converted-constant narrowing* (§9.3): `sema_cce_narrowing_value` is
  clang's `ext_cce_narrowing`, a `-Wc++11-narrowing` warning-as-error
  that recovers with the converted value. `__type_pack_element<-1, T>`
  therefore reports both the narrowing and the index out of bounds; with
  `-Wno-c++11-narrowing` only the latter. noexcept specifiers and
  template arguments recover the same way.
- *Abominable function types*: `__decay` of a cv- or ref-qualified
  function type is the type itself (clang `BuiltinDecay` defers to
  `BuiltinAddPointer`). Forming a pointer or reference to one is
  diagnosed, and in a substitution it is a SFINAE failure.
- *Traits*: `__is_convertible(From, To)` is false when `To` is an
  abstract class (`To test()` is ill-formed, clang
  `CheckConvertibilityForTypeTraits`). A function lvalue binds directly
  to `T&&` ([dcl.init.ref]/5.3.1).
- *Pointer-to-member calls*: `.*`/`->*` on a member function checks the
  ref-qualifier against the object's value category and the cv against
  the object's ([expr.mptr.oper]/6). The call's noexcept-ness comes from
  the member pointer's prototype (`member_call_can_throw`).
- *Current instantiation in template arguments*: an out-of-line member's
  return type rebuilds `typename A<T>::B` inside template arguments
  (`rebuild_ci_type`, e.g. `pair<iterator, bool>`).
- *Elaborated template template parameter*: `friend class TT<Args>;`
  with `TT` a template template parameter names its specialization
  (parser `parse_tag_specifier`).
- *Designators in dependent lists*: a dependent braced-init-list keeps
  its designators in `TemplState::init_designators` (keyed by the
  `InitListExpr`, since the dependent node has no designator storage).
  `subst` re-applies them, substituting `[i]` indices.
- *`&L::m` in a local class of a template*: an implicit member reference
  written `L::m` keeps its qualifier (`DeclRefExtra::qualifier`), and a
  dependent `&L::m` drops its pending `member_refs` entry, so that
  instantiation rebuilds a pointer to member, not `&this->m`. Where
  `this` may be used and `C` is the current instantiation, `C::m` of a
  dependent type is first built as `this->C::m`; its `&` operand is
  rebuilt as the qualified `DeclRefExpr` (`undo_dependent_implicit_member`,
  WP-669; `expr.unary.op/p3-qualified-member-pointer-dependent-type.cpp`).
  The same holds where `N::m` names one member of a class `N` that
  `*this` may derive from only through a dependent base: the
  `this->N::m` built for it records the member
  (`note_qualified_dependent_member`), so `&N::m` is a pointer to member
  even when `N` is unrelated at instantiation (WP-686;
  `expr.unary.op/p3-qualified-member-unrelated-class-dependent-base-run.cpp`).
  An overload set `&N::f` there (`Sema::address_operand`) stays the
  qualified set, resolved at instantiation against the target type
  ([over.over]; WP-715,
  `expr.unary.op/p3-qualified-overload-set-dependent-base-run.cpp`).
  A member function template specialization the pattern resolved,
  `&C::f<A>` or `&Self::f<A>`, keeps its instantiated qualifier when
  `respecialized_member_template` names it again, so it stays a pointer
  to member, not `&this->f<A>` (WP-801;
  `expr.unary.op/p3-member-template-specialization-pointer-to-member-in-template-run.cpp`).
- *Dependent `noexcept(expr)` in a type*: the canonical
  `ExcSpec::DependentNoexcept(profile)` of a function type (as written
  under a pointer, member pointer, reference or GNU `noreturn`) is
  substituted from the profile's operand through `act_on_noexcept_spec`
  ([except.spec]/13, [temp.inst]/14; WP-723,
  `except.spec/p13-dependent-noexcept-gnu-noreturn-subst-run.cpp`).
  Deducing from a conversion function template to a pointer to
  (member) function allows the function pointer conversion that drops
  `noexcept` ([temp.deduct.conv]/5;
  `temp.deduct.conv/p5-noexcept-function-pointer-conversion-run.cpp`).
- *[expr.cond]/4 class unification* (clang `TryClassUnification`): when
  the operands differ in type and either is a class, each is tried
  against the other. The order is: /4.1–4.2, a glvalue target binding
  directly; /4.3.1–4.3.2, same or derived class with at least the
  target's cv (otherwise no conversion); a base class operand is not
  converted to match a derived one at all, even by a converting
  constructor, so `c ? B() : D()` with `D(B)` is a `B` (WP-669,
  tests/conformance/expr.cond/p4-base-not-converted-to-derived.cpp);
  /4.3.3, implicit conversion to the other's prvalue type. If both directions succeed, the result is
  `sema_conditional_ambiguous`; if one does, it is applied. An arm converted
  to a class type (not a reference) has exactly that type, cv included
  (clang's `NoOp` cast): `b ? V() : cref` with `const V &cref` has two
  `const V` arms and is a `const V` prvalue, so `auto &r = b ? V() :
  cref;` binds `const V &` (WP-799,
  tests/conformance/expr.cond/class-prvalue-unified-const-run.cpp).
- *[expr.cond]/6 built-in `operator?:` candidates* (clang
  `FindConditionalOverload`; WP-787): when the arms still differ in type
  after /4 and /5 and either is a class, overload resolution runs over the
  built-in candidates of [over.built]/25-26 (`T(T, T)` for each pointer,
  pointer-to-member and scoped enumeration type an arm converts to;
  `LR(L, R)` for the promoted arithmetic types;
  `add_conditional_builtin_candidates`). The best one converts both arms
  to its parameter types and /7 continues, so two classes converting to
  `bool` give `int`. No best candidate is
  `sema_conditional_ambiguous_ovl`; no viable one leaves the arms to /7,
  which reports them incompatible
  (tests/conformance/expr.cond/p6-class-arms-builtin-overload-run.cpp,
  `-errors.cpp`).
- *[expr.cond]/6 cv-only differing class arms*: arms of one class type
  that differ only in cv-qualification (a `const B` lvalue and a `B`
  prvalue) do not reach the /6 step: /4.3.1 unified them, and the result
  stays a class prvalue even when `B` has `operator bool` (WP-801,
  tests/conformance/expr.cond/p4-cv-differing-class-arms-not-builtin-run.cpp,
  clang's `Parser.cpp` `c ? Diag(..) << x : Diag(..)`).
- *[expr.cond]/7 class prvalue result*: when the arms are a class type
  (after /4) and not glvalues of the same category, each glvalue arm
  copy-initializes the prvalue result (clang `PerformCopyInitialization`
  of an `InitializeTemporary` entity), so `b ? std::move(x) : X(3)` runs
  `X(X&&)` and moves from `x`
  (tests/conformance/expr.cond/class-glvalue-arm-copy.cpp).
- *Calls returning `F&&`*: a call whose result type is an rvalue
  reference to function is an lvalue ([expr.call]/14), so
  `declval<Fn>()()` is callable and `decltype` of it is `F&`.
- *Constructible traits make a temporary* (clang `InitializeTemporary`):
  the initialization `__is_constructible` (and its nothrow and trivially
  forms) checks is bound with `maybe_bind_temporary`. So a private or
  deleted destructor makes the trait false, and a potentially-throwing
  destructor makes the nothrow form false. `maybe_bind_temporary` now
  checks a user-declared trivial destructor too (access, deletion)
  without binding. A call that is a `decltype` operand defers that check
  to `exit_decltype`, which exempts the top-level call.
- *Partial ordering with packs*: `partial_at_least_as_specialized`
  makes an undeduced pack parameter empty (`empty_undeduced_packs`, as
  matching already did). Without this, `C<A>` and `C<A, B...>` were
  ambiguous for `C<int>`.
- *Matching prefilter and cv arrays*: `ArgShape::admits` compares an
  array argument's effective (element) qualifiers, so `S<const F>`
  matches `const int[10]` (libc++ `__copy_cv`, common_reference of
  arrays).
- Tests: `templates/builtins/temp.builtin.type-pack-element-narrowing.cpp`,
  `templates/diag/temp.arg.nontype-narrowing-wno.cpp`,
  `dcl.fct/abominable-compound.cpp`, `expr.mptr.oper/ref-qualified-call.cpp`,
  `templates/builtins/builtin.decay-convertible-functions.cpp`,
  `temp.friend/friend-template-template-param-specialization.cpp`,
  `temp.mem/out-of-line-nested-typename-in-template-args.cpp`,
  `dcl.init.aggr/designated-in-template.cpp`,
  `dcl.init.aggr/reference-member.cpp`,
  `except.spec/noexcept-pointer-to-member-call.cpp`,
  `temp.inst/local-class-member-pointer.cpp`, `expr.cond/class-operand-*.cpp`,
  `expr.call/function-rvalue-reference-result.cpp`,
  `templates/builtins/builtin.is-constructible-temporary-dtor.cpp`,
  `class.temporary/trivial-dtor-checked.cpp`,
  `templates/order/temp.spec.partial.order-empty-pack.cpp`,
  `templates/spec/temp.spec.partial.match-cv-array.cpp`.

**Landed (WP-117): out-of-line members and specialization follow-ups.**
- *Declarator scope*: the parser brackets a qualified declarator's
  suffix with `act_on_enter_declarator_scope` /
  `act_on_exit_declarator_scope` (clang `EnterDeclaratorScope`), so
  `S::vt S::f(vt)` finds `vt` in `S`. The injected-class-name is found
  through that scope, and `typename A<T>::vt A<T>::g(vt)` is rebuilt in
  the current instantiation (`rebuild_in_current_instantiation`) to
  match the in-class declaration, including partial-specialization
  qualifiers `P<T*>::f`.
- *Member specializations*: `template<> int A<long>::s = 9;` explicitly
  specializes the static member (`act_on_explicit_member_var_spec`).
  An explicit specialization (of a function template or a member) is
  never instantiated from the pattern ([temp.expl.spec]/7). A member
  whose body fails to instantiate is marked invalid (clang
  `setInvalidDecl`). `template<> template<class U> A<long>::g(U)` uses
  WP-181's member-template specialization.
- *Invalid bodies*: errors while instantiating a nested function body
  are counted separately (`InstState::body_errors`). They do not make
  the calling statement invalid. A call that names an invalid function
  through a plain `DeclRefExpr` recovers without a follow-on error
  (clang `RecoveryExpr`); member calls and explicit template-id calls
  still report one.
- *Placeholder NTTPs* (`auto*`, `const auto&`, `decltype(auto)`) are
  deduced through `deduce_auto`; a failure is
  `sema_nttp_deduction_failed`.
- *Printer*: a member call keeps its explicit template arguments
  (`s.get<0>()`); sema stores them on the `MemberExpr`
  (`set_member_template_args`).
- Tests: `templates/spec/temp.expl.spec-member-not-instantiated.cpp`,
  `templates/spec/temp.inst-invalid-member-call.cpp`,
  `templates/spec/temp.inst-member-template-args-printed.cpp`,
  `templates/spec/temp.expl.spec-static-member-{run,after-use}.cpp`,
  `templates/nttp/temp.param-placeholder-deduction.cpp`,
  `basic.lookup.unqual/qualified-declarator-run.cpp`.

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
- the reference-workload synthetic corpus, when WP-030 delivers it.

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

**Landed (WP-140): [temp.deduct.partial]/9 reference tie-break.** When
partial ordering deduces in both directions, a parameter pair where both
were reference types is broken as the standard says: an lvalue reference
beats an rvalue reference, otherwise the more cv-qualified referred-to
type is at least as specialized (`templ/order.rs`, `OrderingType.refq`);
one template is more specialized only if it wins some pair and loses
none. This makes the `DoNotOptimize(T&)` / `DoNotOptimize(const T&)`
pair of libc++'s `test_macros.h` unambiguous for a non-const lvalue.

### 11.5 Memoized results and their keys (WP-313)

Every memo is keyed by interned ids only (a `DeclId`, a canonical
`ArgListId` from the type interner, a `NormId`, a partial-spec row
index), so a key is a few words and equal keys mean equal inputs. The
tables are arena-owned (they live as long as the TU's `Sema`) and grow
with the number of distinct keys, never with the number of uses. A hit
must not change diagnostics: a memo that can record a failure keeps
what a later use needs to re-diagnose it (`fn_spec_failures`, the
`atom_cache` records).

| Result | Table | Key |
|---|---|---|
| A class, function or variable specialization (its declaration, chosen pattern and `from_args`) | `TemplateTables` spec list, `find_spec` | (first decl of the template, canonical argument list) |
| The partial specialization a variable or class specialization is instantiated from | `SpecInfo.from` / `from_args` of that specialization | as above: matched once, when the specialization is created (§4.4, §4.11) |
| An alias template specialization's type | `InstState.alias_cache` | (alias template, canonical argument list) |
| Partial ordering of two partial specializations | `TemplState.partial_order` | (row index, row index) |
| A function specialization's first signature-substitution error | `TemplState.fn_spec_failures` | the (invalid) specialization `DeclId` |
| Constraint satisfaction of an entity | `ConceptState.sat_cache` | (entity, canonical argument list) |
| An atomic constraint's value and its notes | `ConceptState.atom_cache` | (atom, canonical arguments of each level it names) |
| Subsumption | `ConceptState.subsume_cache` | (normal form, normal form) |
| A type's identity (hash-consing) | `TypeContext` intern table | the structural key (spec/types.md §7.1) |

Not memoized, by design: explicit-argument substitution into a function
template (each call of `std::declval<T>()` substitutes its explicit
argument again: ~3 report `default` rows per call, 305k in
`simd_copy.pass.cpp` but under 2% of its instantiation time; caching it
would need the SFINAE note replayed per use), default template
arguments (substituted per `check_template_args`, as clang), and
default function arguments (substituted at each use, [temp.inst]/12;
`lazy_defaults` only records which are pending).

WP-313 found the `experimental/simd` slowness was not a missing memo but
two quadratic costs on every lookup and every frame: the intern hash
left a type's second `u32` out of its bucket bits (fixed by folding
FxHash, spec/types.md §7.1), and the per-frame memory meter summed every
lookup overload set (`LookupTables` now keeps the sums as the sets grow,
O(1) per frame). Instantiation counts are unchanged.

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
`kiln++ -fsyntax-only` over the 8 headers. The status page is
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
   **WP-106: measured; below the bar.**
   - *What exists.* Sharing landed with no flag and no new bit in
     WP-070a's substituter. `Substituter::needs_rebuild`
     (`templ/subst/exprs.rs`) reuses an expression by id when
     `deps & (TYPE_VALUE_INST | UNEXPANDED_PACK) == 0`. The LOCAL role
     is played by `INSTANTIATION`: the builder's `decl_deps` sets it on
     every reference to a decl with `DeclFlags::DEPENDENT_CTX`, which
     covers parameters, locals, local classes and lambdas of a template.
     `-fno-share-nondependent` was never added; the doc comment on
     `needs_rebuild` that mentions it is stale (wp-109's file).
   - *Method.* `libcxx_m3.cpp` includes `<type_traits>`, `<bit>`,
     `<limits>`, `<initializer_list>` and `<cstddef>`, then has a few
     `static_assert`s over `numeric_limits`, `bit_ceil`, `rotl`,
     `is_same_v` and `initializer_list`. Clang 19 accepts it. Kiln
     reports 2 errors in both modes ("reference to a member of a
     dependent context", `common_reference.h:160/162`). The run used
     `-fsyntax-only -fkiln-instantiation-report=json`. For "no
     sharing", a temporary env toggle (not landed) forced
     `needs_rebuild` to true. Non-dependent literal leaves (`Integer`,
     `CXXBool`, `String`, …) still had to be shared, because the
     rebuilder has no arm for them ("substitution into IntegerLiteral");
     there were 32 such leaves.

     | mode | instantiations | nodes | `inst.bytes` |
     |---|---:|---:|---:|
     | sharing (default) | 238 | 706 | 27,758 |
     | no sharing (leaves still shared) | 238 | 717 | 27,978 |

   - *Result.* Sharing saves 220 B, or 0.8%. Charging the 32 shared
     leaves at the observed ~20 B per node gives an upper bound of about
     860 B, or 3%. Either way it is far below 15%. The rebuilt bytes are
     dominated by dependent nodes: calls, member accesses and casts on
     template parameters, plus the decls and statements of instantiated
     bodies, none of which can be shared.
   - *Proposal.*
     1. Close Q3 as "measured, below the bar". Do not add
        `ExprDeps::LOCAL` or `-fno-share-nondependent`.
     2. Keep the existing `needs_rebuild` test. It is not an optional
        optimization but the substituter's base case: without it every
        leaf kind would need a rebuild arm. It costs one flag test per
        node.
     3. Fix the stale doc comment.
     4. Re-measure only if a reflection-heavy workload shows
        instantiation memory dominated by non-dependent subtrees.
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
    substitution), and revisit when the clang oracle moves. An error in
    a member typedef of a class template specialization that the
    substitution instantiates is a hard error, not a deduction failure:
    the immediate context ends at the class template's instantiation
    ([temp.deduct.general]/8). Which substitution reaches the
    instantiation first is what the order changes: under clang 22's
    order (constraints first) a failing constraint excludes the candidate
    before its function type instantiates the class, under clang 19's it
    does not, so code that compiles with clang 22 can be the hard error
    with clang 19 and `-fclang-compat=19` (WP-769;
    `tests/conformance/temp.deduct.general/member-typedef-hard-error.cpp`,
    `member-typedef-hard-error-19.cpp`).
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
17. **Q17: Deferred C++26 features**: fold-expanded constraints (P2963)
    and concept template parameters (P2841). **R:** M4/M5, when the clang
    oracle supports them. Structured-binding packs (P1061) and variadic
    friends (P2893) landed in WP-359 against clang 22 (§7.1).

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
| Q17 | P2963, P2841 deferred to M4/M5; P1061, P2893 landed (WP-359). |
| Q3 (2026-10-31) | Measured in WP-106 (§14 Q3): sharing saves 0.8% of `inst.bytes` on `libcxx_m3.cpp` (≤3% counting literal leaves), below the 15% bar. Q3 is closed: sharing stays as the substituter's base case via `needs_rebuild`; no `ExprDeps::LOCAL` bit, no `-fno-share-nondependent` flag. |
