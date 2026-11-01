# spec: kiln_sema

Status: **implemented for M2** (WP-048). Normative for `kiln_sema`
apart from the conversion machinery, which spec/sema-conv.md
(WP-049) covers. §10 lists what is still to be implemented.

Standard: [basic], [conv], [expr], [stmt], [dcl], [class], [namespace].
Reference: clang 19's `Sema`, with diagnostics checked against
`clang++-19 -fsyntax-only` and ASTs against `cargo xtask clang-ast`.

## 1. State
`Sema<'a, 'sm>` holds the semantic state of one translation unit.

There are two lifetimes because `DiagEngine<'sm>` is invariant in
`'sm` and has drop glue: `'a` is the borrow of the engine and `'sm` the
source manager it reports against.

| field | meaning |
|---|---|
| `ast: Ast` | the AST under construction; `kiln_ast` arenas, built through `AstBuilder` |
| `diags` | the diagnostics engine |
| `target: TargetTypes` | builtin type properties (sema-conv.md) |
| `abi: Target` | the full target, used for layout (`sizeof`, `alignof`, bit-fields, constant evaluation) |
| `scopes` | the scope stack (§2) |
| `ctx` | the current semantic `DeclContext` |
| `func: Option<FnState>` | the function body being analysed: declaration, return type, labels, switches, breakables |
| `unevaluated` | depth of unevaluated operands |
| `pending_odr` | DeclRefs of the current full-expression that may be odr-uses (§5.2) |
| `defining` | classes being defined (incomplete until `}`) |
| `enums` | enumerations being defined (`EnumBuild`: running value, best type) |
| `local_externs` | block-scope `extern` variables and functions, keyed by enclosing namespace and name (§3) |
| `exceptions`, `rtti` | `-f[no-]exceptions`, `-f[no-]rtti` (default on; §5.5, §5.6) |
| `braced_args` | braced-init-list call arguments awaiting their parameter types (§6.1) |
| `prototype_params` | the parameters (`PrototypeFrame`: `ParmVar` ids) of each parameter-declaration-clause being parsed (§4.3) |
| `unowned_params` | prototype parameters not yet adopted by a function (§4.3) |
| `declared_returns` | the declared (placeholder) function type of a deduced-return function, for its redeclarations (§4.3.1) |
| `group_placeholders` | placeholder types of instantiation-pattern variables whose declarator group passed the one-deduction check, for the re-check at instantiation (§4.3.1) |
| `pragma_packs` | `#pragma pack` directives seen, with their location and argument text (recorded for the class layout, sema-class.md §6.1) |
| `immediate` | depth of immediate function contexts ([expr.const]/16) entered by `if consteval` and `consteval` bodies (§5.10) |
| `imm` | immediate invocations and immediate-function references of the current full-expression, and why escalated functions are immediate (§5.10) |
| `oven` | the constant evaluator and its budget |
| `classes`, `implicit_declared`, `delayed_access` (+ `delayed_access_depth`), `cleanup`, `extend_collector`, `unparsed_exc` | class state, sema-class.md §8.3 (`FnState` gains `this_ty`/`this_override`, `Scope` gains `nrvo`) |

Constructors:
- `Sema::new(ast, diags, TargetTypes)` picks the layout target from the
  builtin types.
- `Sema::for_target(ast, diags, Target)` takes the target directly.
  The parser uses this one.

## 2. Scopes (`scope.rs`)
- **Kinds:** `ScopeKind` is one of TranslationUnit, Namespace,
  LinkageSpec, Record, Enum, Prototype, Function, Block or Control.
  - `is_local` is true for Function, Block, Control and Prototype.
- **Where names live:**
  - Names in context scopes (translation unit, namespace, record,
    enumeration) live in the AST's per-context lookup tables
    (`AstBuilder::register_name`).
  - Local names live in the scope's own `locals`, which plays the
    role of clang's `DeclsInScope` and `IdResolver` together.
  - Context scopes also record their `using namespace` directives.
- **Who pushes which scope:**
  - **Function:** pushed by `act_on_start_function`. It holds the
    parameters, and is also the body's outermost block, so redeclaring
    a parameter there is an error ([basic.scope.block]/2).
  - **Control:** pushed by the parser for the init-statement and
    condition of `if`, `switch`, `while` and `for`.
  - **Block:** pushed for a compound statement, and for a
    sub-statement that is not one (parse.md §6.1).

## 3. Lookup (`lookup.rs`)
- **Unqualified lookup** ([basic.lookup.unqual]) walks the stack
  innermost first:
  - A local scope searches its `locals`.
  - A context scope searches its entity's table, then the namespaces
    nominated by its `using namespace` directives, transitively, as
    [namespace.udir]/2 requires.
  - If several *different* entities are found at one level, lookup
    reports `reference to 'x' is ambiguous`, with candidate notes.
    The notes follow clang's order (`ambiguity_order`, WP-179): for an
    unqualified lookup that ends at a namespace scope, the names declared
    in that open namespace definition come first, newest first (clang's
    `IdentifierResolver` chain; `Scope::first_decl` marks the first
    declaration created in the scope instance), then the namespace's
    lookup table, and duplicates are removed by moving the last entry
    into the duplicate's slot (`LookupResult::resolveKind`). A qualified
    lookup lists table order.
  - **Identity** ([basic.lookup.general]/1, [class.member.lookup]/3;
    WP-197): declarations of one entity, and type declarations that
    designate the same type (two typedefs of `int`, a typedef and the
    class it names), are one result (`is_ambiguous_lookup` compares
    `designated_type`, canonical, then first declarations; clang's
    `resolveKind` UniqueTypes). A set that is all functions is an
    overload set, and an `UnresolvedUsing*` candidate defers the check
    to instantiation. Member lookup's subobject merge (`class_hit`)
    keys declarations the same way, and its candidate note names a
    record by its canonical type.
  - An ambiguous *type* name (`S s;` with `S` in the namespace and in
    one of its inline namespaces) is diagnosed by the parser through
    `diagnose_ambiguous_name` (spec/parse.md); Sema's recovery
    (`ambiguous_type_recovery`) follows clang: the declaration is
    dropped and the statement skipped.
- **Block-scope using-directives** ([namespace.udir]/2): a local
  scope's `using namespace N;` is collected while walking outwards, and
  `N`'s members are searched at the nearest enclosing namespace (or the
  translation unit) that encloses `N`, together with that scope's own
  using-directives.
- **Qualified lookup** ([basic.lookup.qual]) searches one context's
  table, plus that namespace's using-directives.
- **Argument-dependent lookup** ([basic.lookup.argdep]; WP-068a,
  `overload/adl.rs`, spec/sema-overload.md §2.2): a call's unqualified,
  unparenthesized callee whose ordinary lookup found only namespace-scope
  functions or function templates (no block-scope declaration, no class
  member, no implicit builtin: `call_uses_adl`) also searches the
  arguments' associated namespaces and classes (`assoc_arg`), ignoring
  using-directives. A friend first declared in a class is entered in its
  namespace's table with `LookupFlags::ADL_ONLY`: ordinary and qualified
  lookup skip it, ADL finds it when its class is associated. Results
  already found are compared by first declaration. For overload candidate
  order the unqualified results are listed newest first
  (`overload_name_ref`); `lookup_name` itself keeps storage order.
- **Using-shadows** (§4.8) are looked through. `Found` has two parallel
  vectors:
  - `decls`: the entities, shadows stripped (`AstBuilder::strip_shadow`).
  - `found`: what lookup actually found (the `UsingShadow`, or the
    declaration itself), clang's `FoundDecl`; `Found::shadow(i)` gives
    the shadow, if any.
  - The same entity reached by two paths (a declaration and a
    using-declaration naming it) collapses to one result, the later
    path.
  - An unresolved `using_if_exists` target (`UnresolvedUsingIfExists`)
    is dropped whenever anything else is found.
- **Lookup kinds:** `Ordinary` (non-tags hide tags,
  [basic.scope.hiding]/2), `Tag`, `Nested` (the name before `::`:
  namespaces, namespace aliases and types) and `All` (a
  using-declarator's target: every declaration, no hiding).
- **Namespace aliases** are resolved wherever a namespace is expected:
  `classify_name` reports an alias as `NameClass::Namespace` of the
  aliased namespace, and `act_on_nested_name` resolves an alias
  qualifier. `act_on_nested_name_spelled` also returns the declaration
  as written, for `nns_component`.
- **Redeclaration lookup** (`lookup_current_scope`) searches only the
  current scope. The members of a namespace's inline namespaces are in
  other scopes ([basic.scope.scope]/4, [namespace.def.general]/6): an
  unqualified declaration in the enclosing namespace never redeclares
  one, it declares a distinct entity, and a later use that finds both
  is ambiguous (clang's `FilterLookupForScope` without
  `AllowInlineNamespace`; WP-179). A qualified declaration (`int I::x
  = 1;`, `void N::f() {}`) and the template lookup of an explicit
  specialization (`current_scope_with_inline_dn`, [temp.expl.spec]/3)
  do see them. A qualified *variable* declarator whose lookup finds
  more than one variable or function entity (`int Z::t;` with `t` in
  both `Z` and an inline namespace of `Z`) is ambiguous:
  `redefinition of 't' as different kind of symbol`, note `previous
  definition is here` at the first-declared entity, and the new
  declaration is invalid (`ambiguous_qualified_prev`, clang's
  `MergeVarDecl` on an ambiguous `Previous`; WP-235, test
  tests/conformance/namespace.def/inline-qualified-redefinition.cpp). Redeclaration lookup leaves out using-shadows
  (`current_scope_shadows` returns those, for §4.8's conflict checks). If the current scope is the outermost block of a
  control statement (a Block directly above a Control scope), it also
  searches the Control scope, so `if (int x = 0) { int x; }` is
  diagnosed as [stmt.pre]/5 requires.
- **Block-scope `extern` declarations** are recorded in `local_externs`.
  A later namespace-scope declaration of the same name redeclares them,
  per [basic.link]/6 and clang's local extern decls.
- **`classify_name`** gives the parser a name's category: type,
  namespace, value, or undeclared.
  Landed (WP-140): a non-dependent typedef or alias qualifier is
  replaced by the class or enumeration it names before the lookup, as
  `act_on_nested_name` does, so `typedef D J; J::T x;` declares `x`
  (libc++ `<typeinfo>`'s `__impl::__type_name_t`;
  `tests/conformance/basic.lookup.qual/typedef-qualifier-decl.cpp`).

## 4. Declarations (`decl.rs`, `declspec.rs`)

### 4.1 DeclSpec and Declarator
- The parser fills a `DeclSpec` and a `Declarator` (clang's classes of
  the same names).
- `type_from_declspec_at` checks the specifier combination
  ([dcl.type.general]/2, [dcl.spec]) with clang's wording, and gives
  the base type.
- `Declarator` carries:
  - the name and an optional qualifier
  - `chunks`, innermost first. `Chunk::Function` carries the
    parameters, `variadic`, the parenthesis locations and `exc`, the
    exception specification Sema built from the declarator (§4.7);
    `ExcSpec::None` when none was written (WP-058 added the field)
  - attributes, bit-width, `init_style`, and context
  - `parens` and `after_comma` (spec/parse.md §4): applying the chunks
    wraps the type in `Paren` sugar for each pair of parentheses around
    exactly the chunks applied so far (`declarator_parens`; none around
    the outermost type or a function or array type, spec/types.md)

### 4.2 Declarator to declaration
`act_on_declarator` turns a `Declarator` into a declaration:
1. It applies the chunks from the base type outwards ([dcl.meaning]),
   checking:
   - arrays of references, functions, `void` or abstract types
   - functions returning arrays or functions
   - references to references, pointers to references
   - array bounds: converted constant expressions of integral,
     unscoped enumeration or `_Atomic` integral type, which must be
     positive. A bound that is not a constant expression is a VLA, which
     kiln does not support (`sema_vla_unsupported`), except at namespace
     or class scope, where a bound that `OvenBridge::fold_rvalue_int`
     folds (lax mode: e.g. a `const _Atomic(int)` or `const` variable
     with a non-constant-expression initializer that is still a constant)
     gives clang's `TryToFixInvalidVariablyModifiedType` result: a
     constant array, `-Wvla-cxx-extension` at the bound (with the strict
     evaluation's notes) and the ExtWarn "variable length array folded
     to constant array as an extension" [-Wgnu-folding-constant] at the
     declarator name (WP-165)
   - variable length arrays in a function (WP-183, clang `BuildArrayType`):
     a non-constant bound of a block-scope declarator (variable, typedef,
     alias, type-id) makes a `VariableArray` even when it would fold.
     `-Wvla-cxx-extension` is emitted at the bound with the strict
     evaluation's notes (an array of a VLA, `int k[3][n]`, warns once at
     the outer `[`, without notes). In a GNU dialect (`-std=gnu++NN`,
     `Sema::gnu`) the warning is clang's `ext_vla_cxx_in_gnu_mode`, an
     Extension that is off by default (`sema_vla_cxx_extension_gnu`;
     `-Wall` enables it, `vla-cxx-extension` being in `all`, WP-230).
     `check_vla_var` then handles the variable: `extern` is an error
     ("cannot have 'extern' linkage"); `static`/`thread_local` folds the
     bound (`fold_rvalue_int`) to a constant array with the
     -Wgnu-folding-constant ExtWarn at the name, or is an error ("cannot
     have 'static' storage duration"). Any initializer is the error
     "variable-sized object may not be initialized" at the bound. A VLA
     variable counts as complete. Template substitution rebuilds a
     `VariableArray` with the substituted bound. Kiln's diagnostic
     dedupe keeps an instantiation from repeating the warning (clang
     repeats it), and `sizeof(T[n])` warns once (clang twice).
2. It moves `constexpr`'s implied `const` onto the variable's type.
3. It looks up a previous declaration (§3), checks that redeclarations
   are compatible, and links the redeclaration chain.

The resulting declaration depends on the declarator:

| the declarator is | the result is |
|---|---|
| a `typedef` | `TypedefDecl` |
| an alias declaration | `TypeAliasDecl` |
| a function type | `FunctionDecl` |
| in a record | `FieldDecl` |
| anything else | `VarDecl` |

Redeclaration and linkage rules:
- **Differing types:** a mismatch is
  `redefinition of 'x' with a different type` when the new declaration
  is not `extern`, else
  `redeclaration of 'x' with a different type`. The note says
  `previous definition is here` if the previous declaration was a
  definition, else `previous declaration is here`.
- **Linkage** ([basic.link]): `static`, `extern`, `extern "C"`, members
  of unnamed namespaces, and `const` variables at namespace scope.
- **Block-scope variables** (`merge_local_var`, clang's `MergeVarDecl`;
  [basic.scope.block]/2, [dcl.stc]/5, [basic.link]/6-7): a variable
  redeclaring a variable or parameter of the same block is checked in
  clang's order, stopping at the first error:
  1. an `extern` that also finds a declaration with linkage in the
     enclosing namespace (or an earlier block-scope `extern` of the name)
     when the block's declaration has none: `redefinition of 'x' as
     different kind of symbol`;
  2. differing types (as above; skipped when the earlier declaration is
     invalid, e.g. `static Q(a);` of incomplete `Q`);
  3. `static` after an `extern`: `static declaration of 'x' follows
     non-static declaration`;
  4. non-`static` after `static` (unless an `extern` redeclaring a
     declaration with linkage): `non-static declaration of 'x' follows
     static declaration`;
  5. `extern` after a declaration without linkage: `extern declaration
     of 'x' follows non-extern declaration`;
  6. non-`extern` after a declaration with linkage: `non-extern
     declaration of 'x' follows extern declaration`;
  7. otherwise `redefinition of 'x'`, unless both are `extern`.
  The note is at the earlier declaration (`previous definition` unless it
  is `extern`), even when that declarator was invalid. A new declarator
  that is itself invalid is not merged. A qualified declarator rejected
  in a block (`T(::a);`, "cannot name the global scope") still declares
  `a` in the block, invalid and with linkage (`dropped_qualified_locals`),
  as clang does. Test:
  tests/conformance/basic.scope.block/redeclaration-merge.cpp (WP-235).
- **Exception specifications** ([except.spec]/5): function types that
  differ only in their exception specification are not "different
  types". A new declaration that writes a specification incompatible
  with the previous one's is the error `exception specification in
  declaration does not match previous declaration` with a `previous
  declaration is here` note, and is marked invalid. Compatible
  spellings (`noexcept`, `noexcept(true)`, `throw()`) compare equal
  through the canonical type, and no specification matches
  `noexcept(false)`. A new declaration that writes **none** where the
  previous declaration has one follows "Exception specifications on
  redeclarations" (§4.7).
- **GNU asm labels** (`act_on_asm_label`, clang's
  `ActOn{Function,Variable}Declarator` and `mergeDeclAttributes`): a
  function or variable declarator's label becomes an `AsmLabel`
  attribute, first in the declaration's attribute list, and its
  `StringLitId` is recorded in the `asm_label` cold data of the entity's
  first declaration, which the mangler reads (spec/abi.md). A typedef's
  label is ignored, as clang ignores it.
  - A redeclaration without a label inherits the previous declaration's
    `AsmLabel` (a copy with the inherited flag, ahead of its own
    attributes, as clang orders them).
  - A label differing from the previous declaration's is
    `conflicting asm label`, with a `previous declaration is here` note
    at the old label.
  - A label first written on a redeclaration of an entity that is
    already used is `cannot apply asm label to {variable|function}
    after its first use`.
  - A label on an automatic variable (local, no storage class, not
    `thread_local`) warns `ignored asm label '…' on automatic variable`.
- **Initializing a non-variable** (clang's `AddInitializerToDecl`): an
  initializer on a function or typedef declarator (reachable after an
  asm label, where `{` is not a body) is
  `illegal initializer (only variables can be initialized)` at the
  declaration, which becomes invalid.
- **An `extern` variable with an initializer** is an error at block
  scope; elsewhere it warns `'extern' variable has an initializer`
  (-Wextern-initializer) unless its base element type is const, it is
  `extern "C"`, or it is a template instantiation (clang
  `AddInitializerToDecl`). Explicit and partial specializations warn.

- **Redundant parentheses** (`vexing.rs`, clang
  `warnAboutRedundantParens`, WP-191): when the outermost declarator
  operator is a pair of grouping parentheses (`T(x);`), a block-scope,
  condition or for-init declarator of class or dependent type with only
  a type specifier, a simple name that ordinary lookup finds, no
  function, reference, member-pointer or unbounded array chunk, and no
  preceding comma could be a function-style cast: "parentheses were
  disambiguated as redundant parentheses around declaration of variable
  named 'x'" [-Wvexing-parse] with clang's notes (`add a variable name
  to declare a ...`, unless the class is complete with a trivial
  destructor; `add enclosing parentheses to perform a function-style
  cast`; `remove parentheses to silence this warning`). Otherwise the
  off-by-default "redundant parentheses surrounding declarator"
  [-Wredundant-parens], except when the declarator-id's
  nested-name-specifier begins with the global `::` (`A (::B)`,
  `A (::N::B)`: `Declarator::global_qualifier`, set by the parser; clang
  walks the specifier's prefixes for `Global`). Not in an instantiation.
  Parameter declarators, exception declarations and the abstract
  declarators of type-ids (`sizeof(int (*))`, `W<int (*)>`, a template
  parameter) cannot be expressions and only get -Wredundant-parens
  (WP-235); a parenthesized new-type-id whose declarator has a function
  chunk does not warn (its parentheses are required). Test:
  `tests/conformance/dcl.ambig.res/redundant-parens-abstract.cpp`.
- **Qualified declarator-ids** (`qualified_decl.rs`, clang
  `diagnoseQualifiedDeclaration`, WP-191): a qualified name must be
  declared in a scope that encloses its qualifier's: "definition or
  redeclaration of 'a' cannot name the global scope" (`T(::a);`), "...
  not allowed inside a function", "cannot define or redeclare 'a' here
  because namespace 'N' does not enclose namespace 'M'", and for members
  "non-friend class member 'f' cannot have a qualified name" / "extra
  qualification on member 'f'". The declaration is then dropped, or (for
  the recoverable ones) declared unqualified. Every declarator-id form
  is checked (WP-235): identifiers, operator-function-ids, conversion-
  function-ids (`operator long`), literal-operator-ids (`operator""_y`),
  constructor and destructor names; the scopes print unqualified (`'M'
  does not enclose namespace 'C'`). In a class, a qualified conversion
  function or literal operator is left to the other checks (clang
  rejects both while parsing). A function template explicit
  specialization outside a namespace enclosing its template is "function
  template specialization of 'h' not in a namespace enclosing 'N'" with
  "explicitly specialized declaration is here" (`check_spec_scope`).
  Test: `tests/conformance/dcl.meaning/qualified-declarator-special-names.cpp`.
  **Deviation:** a class template specialization scope prints without its
  arguments (`'TT'`, clang `'TT<int>'`).
- **Default-initialization without a default constructor**
  (`check_class_default_init`, WP-191): a class variable with no
  initializer whose default constructor is missing is resolved as
  `T x()`'s overload set, so the error is "no matching constructor for
  initialization of 'S'" with a note per candidate (clang
  `InitializationSequence` with no arguments).

### 4.3 Functions
- `act_on_start_function` and `act_on_finish_function` bracket a body.
- The body is checked for:
  - redefinition
  - incomplete parameter and return types (with a forward-declaration
    note)
  - falling off the end of a non-`void` function (`falls_off`, the
    clang CheckFallThrough port in `try_stmt/reach.rs`, §7.5). This
    warning is suppressed when the function had errors.
- `main` must return `int` ([basic.start.main]/2). Falling off the end
  of `main` returns 0, so it never warns.
- **Default arguments** (`act_on_param_default_init`,
  [dcl.fct.default]): copy-initialization of the parameter's type,
  from an expression or a braced list, stored in the `ParamInfo` as
  `DefaultArg::Parsed`; `act_on_declarator` merges them with the previous
  declaration's and sets each `ParmVar`'s. (A member function's default
  argument saved for the complete-class context is `DefaultArg::Unparsed`
  and replayed through `act_on_param_default_arg`, sema-class.md §2.7.)
  Calls fill omitted trailing arguments with `CXXDefaultArgExpr`.
- **Trailing return types** (`Chunk::Function::trailing`,
  [dcl.fct]/2): the function's return type, and an error
  (`function with trailing return type must specify return type 'auto'`)
  unless the decl-specifier is a lone `auto`. A return type of array or
  function type ([dcl.fct]/11, `function cannot return function type`)
  is diagnosed at that `auto` type-specifier when the return type is
  trailing (the inner `auto` of a nested `auto (*)() -> F`; also after
  substitution), otherwise at the function declarator's `(`, as clang
  (`dcl.fct/return-function-type-trailing.cpp`).
- **Prototype scope** ([basic.scope.param]): the parser brackets each
  parameter-declaration-clause with `act_on_start_prototype`,
  `act_on_prototype_param(&mut ParamInfo)` and `act_on_end_prototype`.
  `act_on_prototype_param` creates the `ParmVar` at its point of
  declaration (`AstBuilder::prototype_param`, provisionally owned by the
  enclosing context; `ParamInfo.decl`) and pushes it on
  `prototype_params`; unqualified lookup finds it in the rest of the
  declarator: later parameters, default arguments, the exception
  specification and the trailing return type. When the function is
  declared, `create_params` adopts the prototype parameters
  (`DeclArena::set_param_owner`), so the `DeclRefExpr`s in the
  declarator name the function's own parameters; a parameter left
  unadopted (a declarator that is not a function declaration) stays in
  `unowned_params`.
- **Default arguments** ([dcl.fct.default]/9): `check_default_arg_refs`
  reports `default argument references parameter 'x'` for a potentially
  evaluated reference (not inside `sizeof`, `decltype`, … ), on the
  argument as written and on one replayed in the complete-class context.
- **`this` in the declarator** ([expr.prim.this]/2):
  `act_on_start_this_scope(member, quals)` / `act_on_end_this_scope`
  bracket a member function's exception specification and trailing
  return type, where `this` has type `cv X*`; member access into the
  class being defined is allowed there.
- **`-Wvexing-parse`**: a function declarator the parser chose over a
  direct initializer by [dcl.ambig.res]/1 warns as clang does, with
  its fix-it notes (`add a pair of parentheses`, `replace parentheses
  with an initializer`, `remove parentheses`).

### 4.3.1 `decltype` and placeholder types (`deduce.rs`)
- `act_on_decltype(e)` ([dcl.type.decltype]): `DecltypeType` sugar
  over the declared type of an unparenthesized id-expression or member
  access, else `T&&`/`T&`/`T` by value category. The operand is
  unevaluated.
- `auto` and `decltype(auto)` variables ([dcl.type.auto.deduct]): a
  top-level placeholder deduces the decayed, cv-stripped initializer
  type plus the written cv-qualifiers, or `decltype(e)`. A
  placeholder under pointer and reference declarators (`auto *`,
  `const auto &`, `auto &&`) is deduced as for a function call
  ([temp.deduct.call]/2-4) with `auto` as the template parameter.
- Deduced return types ([dcl.spec.auto.general]/9-11): the first
  `return` deduces, later ones must agree (clang's
  `'auto' in return type deduced as ... here but deduced as ...`),
  and a use before deduction is `function 'f' with deduced return type
  cannot be used before it is defined`. A redeclaration
  ([dcl.spec.auto.general]/13) is matched against the declared
  placeholder type (`declared_fn_type`, kept in `declared_returns`), not
  the deduced one, and then adopts the deduced type
  (`adopt_deduced_return`), so `auto f(); auto f() { return 1; } auto
  f();` is valid and `int f();` after it is not.
- Deduced return types in templated functions (WP-122,
  [dcl.spec.auto.general]/12): when the body is in a dependent context
  (`in_template()`), `begin_function_body` gives the pattern the
  return type `dependent_auto(declared)` (clang's AutoType "deduced as
  dependent", printed `auto`, canonical to itself so it mangles `Da`/
  `Dc`) and `deduce_return` checks nothing. The pattern's returns are
  left unconverted, `return;` / `return void();` are not diagnosed
  against the dependent type, and there is no NRVO candidate
  (`copy_elision_candidate`), as in clang. Deduction runs when the body
  is instantiated, so a `return` in a discarded `if constexpr` branch
  never participates (libc++ `__get_comp_type`) and only a real
  conflict between instantiated returns is reported. `return {…}` is
  still an error in the pattern. A call `x.g()` to a member whose
  return type is not yet deduced goes through `check_undeduced_use`
  (instantiating it when it is a specialization member) before
  member-call building.
- Declarator groups ([dcl.spec.auto.general]/3, /8): the parser records
  each declarator of a simple-declaration or member-declaration with
  `act_on_group_declarator(&mut AutoGroup, d)` and closes the group with
  `act_on_end_group(group, &ds)`, which reports, at the end of the group
  as clang does, a function with a deduced return type or a trailing
  return type that is not alone in its group
  (`sema_auto_non_deduced_not_alone`) and placeholders deduced to
  different types (`check_auto_group`). A local group in a template is
  checked again on each instantiation (`group_placeholders`); a class
  template's static members are not (as in clang).
- `-Wunevaluated-expression` ([expr.context]/1; `side_effects.rs`, in
  `-Wmost`): an operand of `sizeof`, `alignof`, `decltype`, `noexcept`
  or an unevaluated `typeid` with side effects (increment, assignment,
  `new`, `delete`, `throw`, `co_await`/`co_yield`, a potentially
  evaluated `typeid`; calls only through their arguments) warns
  `expression with side effects has no effect in an unevaluated
  context`, at clang's expression location. Not in instantiations, not
  for type- or value-dependent operands, not for macro locations.
- **`[[assume(e)]]`** ([dcl.attr.assume]; WP-160, `stmt.rs`
  `act_on_attributed_stmt`). Clang's order is followed for each attribute:
  - On a declaration: `'assume' attribute cannot be applied to a
    declaration`.
  - On a statement that is not null: `'assume' attribute only applies to
    empty statements`.
  - Otherwise, `build_cxx_assume_expr` contextually converts the
    assumption to `bool` and makes it a full-expression. It warns
    `assumption is ignored because it contains (potential) side-effects`
    (`-Wassume`, at the expression's begin location) when
    `side_effects.rs` `has_possible_side_effects` holds. That predicate is
    clang's `HasSideEffects(IncludePossibleEffects=true)`: a call to a
    function that is not `const`/`pure`, a volatile read, a `dynamic_cast`
    to a reference, a non-trivial construction, a temporary with a
    destructor, `new`/`delete`/`throw`, assignment, increment or
    decrement, any dependent or unresolved node, and, through
    default-argument, default-member-initializer and lambda-capture
    initializers, whatever they contain.
  - Before C++23, `-pedantic` adds `use of the 'assume' attribute is a C++23
    extension` (`-Wc++23-attribute-extensions`, which is not under
    `-Wc++23-extensions`).
  - The result is an `AttributedStmt` with one `ASSUME` attribute per
    assumption kept: `a` is the expression and `b` is clang's
    `getExprLoc`. `AttrNode::ASSUME_IGNORED` marks a side-effecting or
    dependent assumption, and codegen and the oven skip those.
  - A dependent assumption is converted and checked again on
    instantiation (`templ/subst/stmts.rs`).
- **`[[likely]]`, `[[unlikely]]`, `[[fallthrough]]`**
  ([dcl.attr.likelihood], [dcl.attr.fallthrough]; WP-182, `stmt.rs`
  `act_on_attributed_stmt`). They are kept in the same `AttributedStmt`
  (clang `ProcessStmtAttributes`), in written order:
  - On a declaration: `'likely' attribute cannot be applied to a
    declaration` (each name).
  - `fallthrough` on a statement that is not null: `'fallthrough'
    attribute only applies to empty statements`; outside a switch of the
    current function (a lambda body has its own): `fallthrough annotation
    is outside switch statement` (clang `handleFallThroughAttr`, checked
    in a template definition). `[[clang::fallthrough]]` is accepted
    (WP-203).
  - **Placement (WP-203; `fallthrough.rs`, clang
    `DiagnoseSwitchLabelsFallthrough`)**: when the body built a
    fallthrough statement (`FnState::has_fallthrough`; an instantiation
    copies them and is not checked, as clang's `HasFallthroughStmt` is
    only set by `ProcessStmtAttributes`), `end_function_body` checks,
    after the return-type check and before the throw-in-nothrow check
    and like them only with no error so far and outside a template,
    that each one directly precedes a switch label: `fallthrough
    annotation does not directly precede switch label` at the
    `AttributedStmt`. A reachable one must be followed, past null
    statements and empty blocks and out of a compound, branch, try block,
    handler or inner switch, by a `case`/`default` label (a loop body
    flows back, a named label starts a new block); an unreachable one
    (`unreachable_attributed_stmts`, every case label reachable, clang
    `fillReachableBlocks`) needs only be the last fallthrough of a
    straight-line run (expression and declaration statements) that
    falls into a label. With no CFG (`if consteval`) nothing is checked.
    The errors come in clang's `SmallPtrSet<_, 8>` order: source order,
    each valid one erased (by its label's order) moving the last element
    into its hole; past 8, source order (clang's is by pointer).
  - **Unannotated fall-through (WP-217; `implicit_fallthrough.rs`,
    clang `DiagnoseSwitchLabelsFallthrough` / `FallthroughMapper`)**,
    run first in `late_analysis` (so after the queued runtime warnings
    and before the placement errors), not in a template definition or
    once an error occurred, but in each instantiation:
    - kiln has no CFG. A label's predecessors are found structurally:
      the statements before it in its block, walked back past empty
      ones (`;`, `{}`, an empty label block, which clang's
      `getLastStmt` counts as the predecessor itself), the branches of
      an `if` ending there (plus the false edge of an `if` with no
      else), and the condition of an `if`/`while`/`for` whose branch or
      body the label starts. A label starting a `do` body (WP-234) is
      entered from the statements before the `do` (walked as above) and
      from the condition (clang's empty loop-back block, whose
      predecessor ends in the `do`), counted unannotated when the
      condition is reached (the body falls off its end or a `continue`
      of that loop is reached) and is not a constant false; the
      placement check likewise lets a `[[fallthrough]];` before such a
      `do` precede that label. Reachability comes from `Sema::stmt_flow`
      (`try_stmt/reach.rs`): the `Reach` walk recording each
      statement's live-in and live-out with every case label reachable
      (clang `fillReachableBlocks`). A switch dispatch, a label directly
      before (`case 1: case 2:`, `L: case 2:`) and an unreachable
      predecessor are good; a reachable `[[fallthrough]];` is
      annotated; anything else is unannotated.
    - At each label with an unannotated predecessor:
      `unannotated fall-through between switch labels`
      [-Wimplicit-fallthrough, off by default, not in -Wall/-Wextra] at
      the `case`/`default`. In an instantiation the instantiation note
      comes next. Then, when no predecessor is annotated and the label
      is not spelled by a macro: `insert '<spelling>;' to silence this
      warning` unless the label's block holds
      only `break`, then `insert 'break;' to avoid fall-through`, each
      with an insertion fix-it at the label. The spelling (WP-234, clang
      `getFallthroughAttrSpelling`) is the name of the latest-defined
      object-like macro in effect at the label whose replacement list is
      `[[fallthrough]]` or `[[clang::fallthrough]]` (before C++17,
      `Sema::std`, the clang spelling is tried first; from C++17 the
      standard one), else `[[fallthrough]]` (`[[clang::fallthrough]]`
      before C++17). The macro history comes from the preprocessor's
      `attr_macros` (spec/pp.md), copied in by the parser through
      `Sema::sync_attr_macros` before each function or lambda body is
      finished; locations are ordered by their include chains, and a
      macro from outside the main file's include tree (predefines, `-D`)
      precedes everything.
    - An unreachable `[[fallthrough]];` that falls into a label (found by
      scanning the unreachable straight-line run backwards): `fallthrough
      annotation in unreachable code` [-Wunreachable-code-fallthrough, in
      -Wunreachable-code] at the attribute, before that label's warning;
      it counts as annotated. Not in an instantiation.
    - Gaps: a label in a range-`for` body, a `try` block or a handler
      is not followed (jumping there is ill-formed, so clang stops at
      the error); -Wimplicit-fallthrough-per-function is not
      implemented; the macro history a function's labels see is the one
      at the end of its body (a later `#define` cannot precede a label,
      so only a lookahead past the `}` is visible, and it is filtered by
      location).
  - A `likely` after an `unlikely` (or the reverse) in the same
    sequence: `'unlikely' and 'likely' attributes are not compatible` at
    the second, with `note: conflicting attribute is here` at the first;
    the second is dropped.
  - `-pedantic` before C++17 (`fallthrough`) or C++20 (`likely`,
    `unlikely`): `use of the 'x' attribute is a C++17/C++20 extension`
    (`-Wc++17-attribute-extensions`, `-Wc++20-attribute-extensions`,
    not under `-Wc++NN-extensions`). A scoped `[[clang::fallthrough]]`
    / `[[gnu::fallthrough]]` (`AttrNode::FALLTHROUGH_SCOPED`) is no
    extension (WP-203).
  - `act_on_if`: both branches `AttributedStmt`s with the same
    likelihood warn `conflicting attributes 'likely' are ignored`
    (`-Wignored-attributes`) at the then-branch attribute, with the
    note at the else-branch one. As clang's `Diags.Report`, it goes
    straight to the engine (`allow_duplicate`, no instantiation notes),
    so each instantiation repeats it, as clang 19 does (checked, WP-203:
    `conflict-per-instantiation.cpp`).
  - Instantiation keeps these attributes unchanged
    (`templ/subst/stmts.rs`); they are not checked again.
  - The parser rejects an argument list on a standard attribute that
    takes none (`carries_dependency`, `fallthrough`, `noreturn`,
    `no_unique_address`, `likely`, `unlikely`, `maybe_unused`, even
    `()`): `attribute 'likely' cannot have an argument list` at the `(`,
    and drops the attribute (clang `ParseCXX11AttributeArgs`).
  - Codegen ignores them: clang 19 consults likelihood only when
    optimizing (no `!prof` weights or `llvm.expect` at `-O0`;
    `tests/codegen/likelihood_attrs.cpp`). The oven evaluates an
    `AttributedStmt` as its statement with one step, and a switch body
    child that is an `AttributedStmt` directly on a `case`/`default`
    label (`[[likely]] case 1:`) is a link in the label chain
    (`attributed_label`), charging its step on the way in, as clang's
    `EvaluateStmt`.
- Braced initializers (`auto x = {1}`, `std::initializer_list`) are
  §10.

**Landed (WP-119).** `decltype(e)::` as a nested-name-specifier
([expr.prim.id.qual]/1). `act_on_nested_name_decltype(ty, loc) ->
Option<(DeclId, NnsId)>` builds `Nns::Type { prefix: None, ty, .. }`
over the `Decltype` sugar (printed `decltype(s)::`, dumped per clang)
and returns the scope it names:
- a non-dependent class or enumeration type: its definition, after
  `require_complete_nns` (an undefined specialization is instantiated,
  an incomplete class diagnosed);
- any other non-dependent type: `'decltype(i)' (aka 'int') is not a
  class, namespace, or enumeration` (`sema_expected_class_or_namespace`,
  at the `decltype`), `None`;
- a dependent type: one shared dependent marker per TU
  (`TemplState::decltype_scope`, an implicit unnamed `TypeAlias` of
  `QualType::DEPENDENT` made by `AstBuilder::unlinked_alias`), for which
  `is_dependent_ctx` is true. Names after it therefore take the existing
  dependent paths (`DependentNameType`, `DependentScopeDeclRefExpr`,
  `Nns::Identifier`), and the Substituter's `Nns::Type` arm substitutes
  the decltype and resolves the scope at instantiation (`nns_context`:
  `type 'decltype(int{})' (aka 'int') cannot be used prior to '::'`).
- The Substituter keeps the written keyword of a substituted
  `DependentNameType` (`typename S::type`, as clang's
  `ElaboratedType`); it was dropped before.

### 4.3.2 Class template argument deduction (`templ/ctad.rs`, WP-071b, WP-134)
[dcl.type.class.deduct], [over.match.class.deduct], [temp.deduct.guide].

**Implemented.**
- **Guides parse** at namespace and class scope: `[explicit] X(params)
  -> X<…>;`, as a template or not, with a requires-clause. The
  declarator name is `SpecialName::DeductionGuide(template)`, the
  declaration a `CXXDeductionGuide` (`act_on_deduction_guide`). Guides
  are found by lookup of that name in the template's context
  (`user_guides`).
- **Placeholder.** A class template name used as a type specifier is a
  `DeducedTemplateSpecialization` (DTS) type with no deduced type
  (`ctad_placeholder`). The parser accepts one in a simple-declaration,
  after `new`, and before `(`/`{` in an expression (functional cast).
- **Sites.** Each site calls `deduce_ctad(declared, args, braced, how,
  loc)` with `how` one of `CtadInit::{Direct, DirectList, Copy, CopyList}`:
  - variables (`act_on_init`; `act_on_uninit` for `X x;`);
  - `new X(…)`, `new X{…}`, `new X`;
  - `X(…)`, `X{…}`, `X()` (`ctad_cast_type`,
    `act_on_ctad_list_cast_type`).

  The result is the DTS with the deduced type (printed as the
  specialization, as clang), with the declared cv-qualifiers.
- **Guide set** (`ctad_guides`). The implicit guides are cached per
  template in `TemplState::ctad_guides` once the class has a
  definition; `TemplState::ctad_origin` records each one's origin
  (`GuideOrigin::{Ctor, CtorTemplate, Copy, Aggregate}`). The set is:
  - one implicit guide per non-template constructor of the primary
    definition (a hypothetical `X()` when there is none). An implicit
    guide is a detached `FunctionTemplate` over the class template's
    own parameter list (`AstContext::detached_template_decl`) with
    return type `X<T…>`, the constructor's parameters (names) and
    trailing requires-clause. A parameter with a default argument gets
    a source-less `OpaqueValueExpr` of its type as the guide's default
    (`guide_default_placeholder`, WP-152; clang's
    `transformFunctionTypeParam`, printed `<null expr>`): the guide has
    the constructor's arity but its default arguments are never
    substituted or evaluated (the constructor's expression names the
    constructor's template parameters, not the guide's). Substituting a
    guide specialization's declaration (`Substituter::default_args`)
    gives it another placeholder of the substituted type;
  - one guide per constructor template (`ctor_template_guide`, WP-134):
    the class parameters followed by the constructor template's,
    re-depthed one level shallower and renumbered after them
    (`ctad_redepth_param`; the class level is substituted by null
    arguments, which retain it); its constraints are the class
    template's and the constructor template's, conjoined;
  - the copy deduction candidate `X(X<T…>) -> X<T…>`;
  - P1816 (WP-134): when `X` has no user guides and the initializer is
    a non-empty braced or parenthesized list, the aggregate deduction
    candidate (`aggregate_guide`): one parameter per element the
    initializers initialize (bases, then fields), brace elision assumed
    only for a non-dependent sub-aggregate or an array whose bound is
    not value-dependent; a braced element or string literal for an
    array of dependent bound is `T(&&)[N]` / `const T(&)[N]`. Not cached: it depends on the initializer. Call
    deduction deduces `P'[N]` from a braced list (each element deduces
    `P'`, the length `N`);
  - the user guides (`user_guides`).
- **Member templates** (WP-134). A member class template of a class
  template specialization (`Outer<int>::In`, redeclared at
  instantiation, [temp.mem]) has no definition until one of its
  specializations is instantiated. Its implicit guides
  (`member_ctor_guides`) take the original member's non-template
  constructors, their parameter types substituted with
  `base_levels(tmpl)` plus the redeclared template's own parameters as
  arguments. Its member deduction guides are redeclared with it: the
  Substituter maps a `DeductionGuide(t)` name to the instantiated `t`.
  Each constructor template of the original member gives a guide
  (WP-152, `ctor_template_guide` with the member levels): the
  redeclared template's parameters followed by the constructor
  template's, retained at the redeclared template's depth.
  The member is named as a placeholder through an alias of the
  specialization (`using O = Outer<int>; O::In`) or through the
  template-id itself (`Outer<int>::In`, WP-152): the lookahead finds
  `In` in `Outer`'s primary pattern, so `parse_ctad_placeholder`
  classifies the name again in the context the parsed
  nested-name-specifier names and takes the instantiated member.
- **Alias templates (P1814, WP-134).** For `A<…>` naming an alias
  template whose defining-type-id is a simple-template-id `C<…>`, each
  guide `g` of `C` is transformed (`build_alias_guide`): `g`'s return
  type is deduced from the RHS (packs deduce element-wise,
  `DeducedArgs::expansions`); the alias parameters used by the
  deductions are kept (renumbered when only some are); the deductions
  are resugared to them; `g`'s undeduced parameters follow. The guide
  carries `ctad_alias[g'] = (alias, rhs)`, and `deduce_call` checks
  clang's `__is_deducible(A, R)` on the deduced return type
  (`note_alias_not_deducible`). An RHS that is not a simple-template-id
  is `sema_ctad_non_deducible_template`.
- **Resolution.** The usual candidate machinery with `deduce_call`, then
  `order_template_candidates` and `best_viable`.
  - List-initialization runs [over.match.list] phase 1 first (WP-134):
    the initializer-list guides (first parameter (reference to cv)
    `std::initializer_list<E>`, dependent or not; any others defaulted;
    `is_init_list_guide`) with the whole list as one argument. A
    viable or ambiguous phase 1 decides; otherwise phase 2 runs all
    guides on the elements. A single element whose type is a
    specialization of `X` or derived from one skips phase 1
    (`ctad_list_of_spec`).
  - In copy-initialization (`=` and `= {…}`), explicit guides and
    constructors are non-viable (`CandidateFailure::Explicit`).
  - In copy-list-initialization, an explicit winner is
    `sema_ctad_explicit_copy_list` plus `note_ctad_explicit_here`.
  - Tie-breakers in `overload/best.rs`
    ([over.match.best.general]/2.11-2.13): a user guide beats an
    implicit one; the copy deduction candidate beats the others; a
    non-template constructor's guide beats a constructor template's.
- **Diagnostics** use clang's wording: `sema_ctad_no_viable`,
  `sema_ctad_ambiguous`, and the explicit-copy-list error and note
  above. Each implicit-guide candidate note is followed by
  `implicit deduction guide declared as '<sig>'`
  (`implicit_guide_signature`; parameter names as clang's DeclPrinter
  places them, `Ts ...xs`, and an inherited default argument as
  `= <null expr>`).
- **Array new.** `new X[n]{…}` with a placeholder element type is
  `sema_ctad_compound_type` ("cannot form array of deduced class
  template specialization type") at the innermost bound, as clang.
- **AST dump.** A user-declared `CXXDeductionGuideDecl` is compared
  against clang (spec/ast-dump.md); implicit guides are not dumped.
- **Dependent initializers.** A type-dependent argument gives the DTS
  deduced as `QualType::DEPENDENT`. The Substituter maps it back to the
  undeduced placeholder, so deduction is redone for the instantiation.
  A non-dependent use in a template is deduced at definition.

**Deferred (no candidate, or diagnosed as unsupported).**
- Inherited-constructor guides (P2582): clang 19 does not implement
  them either, so `D d(args)` through `using B<T>::B;` finds only `D`'s
  own guides, as clang 19 does. Implementing it is the alias transform
  of `B`'s guides with `D`'s base-specifier as the right-hand side.
- Guides from constructor templates of a member class template of an
  instantiated class template (only its non-template constructors and
  user guides are used).
- Qualified CTAD through a template-id, `Outer<int>::In x(…)`: the
  parser classifies `In` as a dependent name, giving a variable of a
  dependent type with no diagnostic (name classification, not CTAD;
  `using O = Outer<int>; O::In x(…)` works).
- `sema_ctad_deleted` (a deleted constructor selected) and
  `sema_ctad_not_allowed` (a placeholder in a non-deducible position)
  are reserved ids that nothing emits yet.

### 4.4 Tags, enumerations, bit-fields and anonymous records
- **`act_on_tag(kind, name, TagUse, …)`** implements [dcl.type.elab]
  and [class.name]:
  - A reference finds or injects the tag. An unqualified reference
    whose lookup finds tags of two namespaces (an inline namespace's
    and its parent's, [namespace.def.general]/7) is "reference to 'X'
    is ambiguous" with candidate notes, recovering with the last tag;
    the parser reports a class template-id `X<int>` the same way.
  - `struct N::X;` (or an opaque `enum N::E : int;`) outside an
    explicit specialization or instantiation is "forward declaration
    of struct cannot have a nested name specifier" at the qualifier
    ([dcl.type.elab]/1, [dcl.enum]/1), reported before the lookup.
  - A declaration redeclares the tag in the current scope.
  - A definition checks for redefinition and kind mismatch.
- **Enumerations** ([dcl.enum]):
  - enumerators are typed as clang types them
  - a fixed underlying type gets converted-constant-expression checks
    and narrowing diagnostics; a non-constant initializer is
    `enumerator value is not a constant expression` (clang
    `CheckEnumConstant`, WP-230)
  - an unfixed one's initializer is an integral constant expression that
    may fold (`fold_not_ice`, below)
  - an unfixed one gets its type from the enumerators' range (the best
    type), and the enumerators are then retyped to the enumeration
  - `replace_enumerator_init` records the converted initializer
- **Bit-fields** ([class.bit]):
  - the width must be an integral constant expression; one that folds
    laxly is accepted (`fold_not_ice`, below)
  - it must not be negative
  - it must not be zero-width with a name
  - it must not exceed the type's width
  - `mark_bitfield` records the width
- **Anonymous unions and structs** ([class.union.anon]):
  - `act_on_decl_spec_only` creates the unnamed member, or the implicit
    variable outside a class, and returns it.
  - `IndirectField`s inject the members into the enclosing scope.
  - Conflicts are diagnosed (`member of anonymous union redeclares 'x'`).
  - Anonymous unions outside a class must be `static` at namespace
    scope.
  - An anonymous struct outside a class is an error.
  - `act_on_finish_record_definition(r, then_semi)` sets `ANONYMOUS`
    and `HAS_VARIANT_MEMBERS`.
- **Unnamed tags named by a typedef** record their typedef with
  `set_typedef_for_anon` ([dcl.typedef]/9).

### 4.5 Attributes
- `act_on_attribute(syntax, scope, name, args, loc)` maps a parsed
  attribute to an `AttrKind`, or warns and returns `None`.
- **Recognised kinds:**
  - `nodiscard`/`warn_unused_result`
  - `maybe_unused`/`unused`
  - `deprecated`, `noreturn`, `fallthrough`
  - `likely`/`unlikely`
  - `no_unique_address`
  - `alignas`/`aligned`, `packed`
  - `used`, `noinline`, `always_inline`, `weak`, `const`, `pure`, `cold`,
    `hot` (`AttrKind::HOT`, WP-172)
  - `using_if_exists` (`__attribute__((using_if_exists))` or
    `[[clang::using_if_exists]]`; other `clang::` attributes are still
    ignored), on using-declarations (§4.8)
  - `exclude_from_explicit_instantiation` (GNU syntax or `clang::`,
    `AttrKind::EXCLUDE_FROM_EXPLICIT_INSTANTIATION`, WP-214): an explicit
    instantiation definition of the enclosing class does not instantiate
    the member (`instantiate_members_of`), and codegen keeps it an
    implicit instantiation (spec/codegen.md §7h). It is not copied to
    instantiated members; `Ast::excluded_from_explicit_instantiation`
    follows the pattern chain.
  - `ext_vector_type(N)` / `vector_size(N)` (GNU syntax; `vector.rs`).
    They take exactly one expression and are consumed into the declared
    type, never attached. `ext_vector_type` applies only to a typedef or
    alias-declaration (`'ext_vector_type' attribute only applies to
    typedefs`); `vector_size` also to the type of a variable, field or
    parameter. Among the decl-specifiers they modify the decl-specifier
    type (so a function can return a vector); after a declarator, the
    whole declared type (on a function, pointer or array declarator:
    an invalid element type, after which the type is left unchanged,
    as clang). The element
    type must be integral or floating (`vector_size`: a non-`bool`
    builtin), else `invalid vector element type 'T'`; N must be an
    integral constant (`ext_vector_type attribute requires an integer
    constant`), nonzero (`zero vector size`) and, for `vector_size`, a
    multiple of the element size (`vector size not an integral multiple
    of component size`). A value-dependent N (or a dependent element of
    a `vector_size` vector) gives a `DependentVector` (spec/types.md);
    after substitution it is diagnosed at the size operand (clang: at
    the attribute name).
  - GNU `malloc` (function with a pointer result, else `'malloc'
    attribute only applies to return values that are pointers`; on a
    non-function `... only applies to functions`), `alloc_size(n[, m])`
    and `alloc_align(n)` (`vector.rs::alloc_attribute`, checked in
    `decl.rs::check_alloc_attr` as clang's `handleAllocSizeAttr` /
    `AddAllocAlignAttr`): integer-constant one-based indices (`requires
    parameter N to be an integer constant`; arity: `takes at least 1
    argument` / `takes no more than 2 arguments` / `takes one argument`)
    naming an integer parameter (`parameter N is out of bounds`, `argument
    may only refer to a function parameter of integer type` at the
    argument, a dependent parameter included; index 1 of an implicit
    object member function: `is invalid for the implicit this
    argument`); the result must be a pointer (`alloc_align`: or a
    reference), else a `-Wignored-attributes` warning; on a non-function
    `only applies to non-K&R-style functions`. Codegen: spec/codegen.md
    "Allocation functions".
  - `flag_enum` and `enum_extensibility(open|closed)` (GNU syntax or
    `clang::`, `AttrKind::FLAG_ENUM` / `ENUM_EXTENSIBILITY`, WP-230), on
    enumerations (else `'x' attribute only applies to enums`
    [-Wignored-attributes]). `flag_enum` takes no arguments;
    `enum_extensibility` takes exactly one, an identifier (the parser
    passes a leading identifier as `AttrArg::Ident`, clang
    `attributeHasIdentifierArg`): `takes one argument`, `requires
    parameter 0 to be an identifier`, and for another identifier
    `'enum_extensibility' attribute argument not supported: 'x'`
    [-Wignored-attributes]; an argument already in error drops it
    silently. They change only the switch checks (§7 Statements). Not
    yet: -Wflag-enum (an enumerator out of range of the flags).
- **Ignored with a warning:**
  - unknown attributes: `unknown attribute 'x' ignored`. Scoped names
    print without their scope, as clang 19 does.
  - GNU `visibility`, `section` and `format`: `'x' attribute ignored`
- **Argument encoding** (`Attr.a`, `Attr.b`; see spec/abi.md §3.1):

  | kind | `a` | `b` |
  |---|---|---|
  | ALIGNED | `ExprId` of the alignment expression | raw `QualType` of `alignas(type-id)` |
  | NODISCARD, DEPRECATED | `ExprId` of the string-literal message | — |
  | ASM_LABEL | raw `StringLitId` of the label (flags bit 1: inherited) | — |
  | BUILTIN | the builtin id (`kiln_ast::decl::builtin`; implicit, §5.4) | — |
  | MALLOC | — | — |
  | ALLOC_SIZE | `ExprId` of an `IntegerLiteral` holding n | same for m, or 0 |
  | ALLOC_ALIGN | `ExprId` of an `IntegerLiteral` holding n | — |
  | ENUM_EXTENSIBILITY | 1 for `open`, 0 for `closed` | — |

- **Semantics:**
  - `nodiscard` applies to functions, records and enumerations. A
    discarded call, or a discarded by-value result of such a type,
    warns. The callee is clang's `getReferencedDeclOfCallee` (WP-203):
    a name or member access (`m.f()`, `p->f()`, `m.operator-(1)`, a
    static member through an object), under parens, implicit casts and
    substituted template parameters, through `.*`/`->*` (the right
    operand) and unary `*`/`&`/`+`. A call without one never warns. The
    result type's attribute wins over the callee's (clang
    `getUnusedResultAttr`); a call returning a reference has no by-value
    result. Not for a `void` call. Reported at the callee's start.
  - `deprecated` warns on use: a name, and a member named by a member
    access or an implicit `this->` (clang `DiagnoseUseOfDecl` in
    `BuildMemberReferenceExpr`, WP-203). Unlike clang, a non-dependent
    use in a template is not diagnosed again at instantiation (nor is
    a non-dependent discarded nodiscard call).
  - `maybe_unused` suppresses unused warnings.
  - `alignas` is validated as a power of two and fed into layout.
  - `act_on_tag_attrs` attaches class-head and enum-head attributes.

### 4.6 Namespaces, linkage specifications, static_assert
- **Namespaces:**
  - Named namespaces are reopened ([namespace.def]).
  - An unnamed namespace is created once per scope and gets an
    implicit `UsingDirectiveDecl` ([namespace.unnamed]).
- **Using-directives:** the payload is `Payload::None(nominated.raw())`.
- **`extern "C"`/`"C++"`** creates a `LinkageSpecDecl`.
  - Without braces (`extern "C" int x;`) the parser calls
    `act_on_linkage_spec_braceless`, which sets
    `ScopeData::LINKAGE_BRACELESS` in the payload's `aux`.
    `Decls::is_single_line_linkage(d)` is true for the declaration it
    directly contains.
  - [dcl.link]/8: that declaration is treated as if it had `extern`
    when deciding whether it is a definition (`is_var_definition`, the
    redefinition checks in `act_on_var`, codegen's `emit_global_var`).
    The storage class itself stays `None`, so the AST dump shows no
    `extern`, as in clang. `extern "C" int x; extern "C" int x = 1;`
    is a declaration followed by its definition.
  - [dcl.link]/6-7 (`extern_c.rs`): `Sema::extern_c` maps each name
    declared as a C-linkage function or variable (non-template,
    non-local) to its latest declaration. An unqualified function or
    variable declared with C language linkage that finds no
    redeclaration by ordinary lookup links to that entity's redecl
    group, so two `extern "C" void f();` in different namespaces are
    one entity (lookup through both using-directives is then not
    ambiguous). A function whose parameters differ is "conflicting
    types for 'f'" (the note names the first declaration; a
    variable's names the latest, as in clang). A global-scope variable
    and a C-linkage entity of the same name in a namespace conflict
    either way (`err_extern_c_global_conflict` and its note); a
    global function does not.
- **Redefinition through an initializer** (clang
  `AddInitializerToDecl`): a namespace-scope variable that was a
  declaration before its initializer (`extern`, or a single-line
  linkage specification) gets "redefinition of 'x'" with "previous
  definition is here" if an earlier redeclaration is a definition. The
  variable becomes invalid and the initializer is dropped.
- **`static_assert`** ([dcl.pre]/10) requires a converted constant
  expression of type `bool`.
  - On failure it reports
    `static assertion failed due to requirement '…'`, with the
    condition printed by `TypeNames::write_expr`, plus the message if
    there is one.
  - Evaluation notes follow (§8).

### 4.7 Exception specifications (`except.rs`)
The parser calls these while parsing a function declarator, and stores
the result in `Chunk::Function::exc`; the function type keeps the
specification as written (`TypeNames` prints ` noexcept`,
` noexcept(<expr>)`, ` throw()`), the canonical type folds it to
`Nothrow` or `None`.
- **`noexcept`** is `ExcSpec::BasicNoexcept`.
- **`act_on_noexcept_spec(e, loc, start)`** ([except.spec]/2): `e` is
  a contextually converted constant expression of type `bool`
  ([expr.const]/12), evaluated with `EvalContext::converted(start)`.
  The result is `NoexceptExpr { expr: ConstantExpr(e'), value }`.
  Diagnostics point at `start`, the operand's first token, and follow
  clang 19:
  - an integral or unscoped-enumeration value other than 0 or 1:
    `noexcept specifier argument evaluates to 2, which cannot be
    narrowed to type 'bool'` (an error here; clang's is a default-error
    `-Wc++11-narrowing` warning);
  - a type whose conversion to `bool` is not allowed in a converted
    constant expression (pointers, `std::nullptr_t`, floating point):
    `conversion from 'T' to 'bool' is not allowed in a converted
    constant expression`, with `T` as written (`int *const`);
  - a class or scoped-enumeration type: `value of type 'T' is not
    implicitly convertible to 'bool'`;
  - a non-constant operand: `noexcept specifier argument is not a
    constant expression`, plus the Oven's notes.

  After an error the specification is `noexcept(false)`, as clang
  recovers. A dependent operand would give `NoexceptValue::Dependent`
  (no templates in M2).
- **`act_on_dynamic_exception_spec(types, ok, loc, rparen)`**:
  `throw()` is `DynamicNone`, accepted silently (clang 19 accepts it
  in C++26 although P1152/C++20 removed it from the grammar). A
  non-empty list is `ISO C++17 does not allow dynamic exception
  specifications` (`-Wdynamic-exception-spec`, default error) with a
  `use 'noexcept(false)' instead` note, then `Dynamic(types)`.
- **Gaps:** `__attribute__((nothrow))` is not parsed.

**Landed (WP-112).** Member functions, lazy instantiation and the
"needed" points ([except.spec]/13):
- *Complete-class context* ([class.mem.general]/7): a member
  declarator's *noexcept-specifier* is replayed after the class.
  `defer_late_noexcept(f, d)` gives `f`'s type `ExcSpec::Deferred(f)`
  and puts `f` in `unparsed_exc`. The parser brackets the replay with
  `act_on_start_delayed_exception_spec(f) -> bool` (a `this` scope with
  `f`'s cv-qualifiers unless `f` is static, [expr.prim.this]/2) and
  `act_on_end_this_scope`. `act_on_exception_spec(f, spec)` then sets
  the written spec on every redeclaration (`set_member_exception_spec`).
  `throw()`, `noexcept` and `noexcept(expr)` all go through this path.
- *Needed*: `exc_spec_needed(f, loc)` runs at every use that marks a
  function referenced, including unevaluated operands such as
  `noexcept(f())`, as clang's `MarkFunctionReferenced` does. A spec
  that is still unparsed gives `exception specification is not
  available until end of class definition`
  (`sema_exception_spec_not_parsed`). Otherwise `resolve_exception_spec(f,
  at)` runs; it now takes the point of instantiation `at`.
  `resolve_exc_for_type(f, loc)` resolves the spec before a
  `DeclRefExpr` or a `&C::m` member pointer takes `f`'s type, because
  the spec is part of the type (clang's `DiagnoseUseOfDecl`).
- *Lazy dependent specs* ([temp.inst]/14, clang `EST_Uninstantiated`):
  when a member of a class template specialization is instantiated and
  its `noexcept(expr)` is value-dependent, and the member is neither a
  member template nor in a still-dependent context (`lazy_exc_spec`),
  the member gets `ExcSpec::Deferred(pattern)` and an entry in
  `uninstantiated_exc: FxHashMap<DeclId, DeclId>` (first declaration to
  pattern). `resolve_exc` calls `instantiate_exception_spec(f, poi)`
  first. That function substitutes the pattern's expression in the
  member's context, with its parameters and `this`, under a
  `FrameKind::ExceptionSpec` instantiation frame. Errors carry `in
  instantiation of exception specification for 'f' requested here`
  (`note_template_exception_spec_here`). A failed substitution leaves
  the member potentially throwing (`ExcSpec::None`), as clang does.
- *Redeclarations*, overriders more lax than the base, and
  [conv.fctptr] function-pointer conversions use clang's wording. The
  tests are in `tests/conformance/except.spec/`.
- **Exception specifications on redeclarations** (WP-268,
  `merge_missing_exc_spec`; clang 19's missing-specification path of
  `Sema::CheckEquivalentExceptionSpec`). A function redeclaration
  (including a global `operator new`/`delete`) that writes no
  exception specification where the previous declaration `p` (the
  latest one redeclaration lookup found) has an incompatible one is
  decided by the first matching row:

  | Condition | Result | The redeclaration's type gets |
  |---|---|---|
  | `p` is non-throwing, the entity has C language linkage (its first declaration is in an `extern "C"` context), and `p` is in a system header, has no location, or the first declaration is a builtin (implicit `builtin_id`, or recognized as a `builtin::LIB` library function, §5.4) | accepted silently (C libraries may add the specification: libc++'s `extern "C" double lgamma_r(double, int*);` after glibc's `__THROW` one) | `throw()` |
  | the redeclaration is a replaceable global allocation function ([new.delete], `is_replaceable_global_alloc`) and `p`'s specification is not value-dependent | `ExtWarn` `'f' is missing exception specification 'S'` (`sema_missing_exception_spec_ext`, `-Wmissing-exception-spec`, on by default, suppressed in system headers like any warning) | `p`'s |
  | otherwise | error `'f' is missing exception specification 'S'` (`sema_missing_exception_spec`); the redeclaration is invalid | `p`'s unless value-dependent or deferred |

  `S` is `p`'s specification as written (`noexcept`, `throw()`,
  `noexcept(<expr>)` printed as clang prints it). Both diagnostics take
  a `previous declaration is here` note and clang's fix-it inserting
  ` S` just past the declarator's `)` (none with a trailing return
  type, and, as kiln does not record where a cv- or ref-qualifier ends,
  none after either). "`p`'s" specification is inherited as clang does:
  `noexcept(true)` becomes `noexcept`, `noexcept(false)` none, others
  as written. Only `p`'s location counts, not the redeclaration's: a
  second redeclaration in user code follows the first (now `throw()`,
  not in a system header) and is the error `... 'throw()'`. The
  inherited specification is what `noexcept(f(...))` and codegen's
  `nounwind` see. The global `operator delete` forms never reach the
  table (a deallocation function without a specification is implicitly
  `noexcept`, [except.spec]/11). The function name in these and the
  other redeclaration diagnostics prints word operators as clang does
  (`operator new`). Tests: `tests/conformance/except.spec/redecl-missing-*.cpp`
  (the C-library case with an `-isystem` header in `Inputs/`).
- *Diagnostics*: `TypeNames::aka` gives no `aka` when the written and
  canonical spellings differ only in exception specifications, matching
  clang's `desugarForDiagnostic`. The Oven's `this` trap says `implicit
  use of 'this' pointer ...` for a member named without `this->`.
- Implicit and defaulted special members keep `Deferred(f)` and are
  resolved through `class/special/excspec.rs` (sema-class.md §4.7). The
  `noexcept` operator sees these specs through constructor, destructor
  and member calls (§5.3).

### 4.8 Using-declarations, `using enum`, namespace aliases (`using.rs`)
- **Using-declarators** ([namespace.udecl]):
  `act_on_using_declarator(using_loc, qual, name: DeclName, name_loc,
  if_exists, ellipsis)` handles one declarator of `using a::x, b::y;`.
  The name may be an operator or conversion function name (WP-149).
  A declarator with `...` ([namespace.udecl]/1,
  P0195) is a pack expansion over a dependent qualifier, expanded
  per element when the class template is instantiated
  (`templ/class.rs` `member_using`).
  - The name must be qualified: `using declaration requires a
    qualified name`.
  - The target is looked up with `LookupKind::All` in the qualifier's
    context.
  - Nothing found: `no member named 'x' in namespace 'N'`, unless the
    declaration has `[[clang::using_if_exists]]` (§4.5). Then an
    `UnresolvedUsingIfExists` declaration is introduced in place of a
    target, and only a use of the name is an error: `reference to
    unresolved using declaration`, with a note at the declaration.
  - A namespace target ([namespace.udecl]/3): `using declaration cannot
    refer to a namespace`, with a note suggesting `using namespace`.
  - A class qualifier at namespace or block scope ([namespace.udecl]/8)
    is allowed only for enumerators; otherwise `using declaration
    cannot refer to class member`, with clang's note suggesting an alias
    declaration (types) or a reference (variables).
  - On success, a `Using` declaration (not entered in lookup) records
    the qualifier, and one `UsingShadow` per declaration found is entered
    in the current scope (a local scope's `locals`, or the context's
    table).
  - `using typename q::x;` ([namespace.udecl]/1, /12;
    `act_on_using_typename_declarator`, WP-169): with a dependent
    qualifier in a class member specification, the declarator is an
    `UnresolvedUsingTypename` member `x` (entered in lookup; a type
    name) whose type is its own dependent `UnresolvedUsing` type
    (spec/types.md §5.3, WP-179); at instantiation (`templ/class.rs`
    `member_using_typename`) it is redone as an ordinary
    using-declarator, and the `UnresolvedUsing` type in instantiated
    members resolves to the type of its shadow's target
    (`subst/types.rs` `decl_type`). With a non-dependent qualifier, or
    at instantiation, a target that is not a type reports `'typename'
    keyword used on a non-type` at the name, with the `target of using
    declaration` note (clang's `err_using_typename_non_type`).
  - Dependent using-declarators in templates (WP-179):
    - A class member `using B<T>::m;` is an `UnresolvedUsingValue`
      member (spec/sema-class.md). An unqualified use of `m` in the
      class (`act_on_id_expression_dn`) is a baseless
      `CXXDependentScopeMemberExpr`, also when the class's own
      overloads of `m` join the set (no ambiguity, and the pattern's
      own `f(char)` is not a redefinition of the unresolved `f`,
      `overload/redecl.rs`). The instantiation (`subst/exprs.rs`
      `implicit_dependent_member`) looks the name up again from the
      instantiation context: an implicit member access or a static
      member reference.
    - An `UnresolvedUsingTypename` is a dependent context, so
      `type::value` and `typename type::vt` nest through it.
    - At block scope in a template, a declarator with a dependent
      qualifier (`using E<T>::A;`, [namespace.udecl]/8: an enumerator)
      is a local `UnresolvedUsingValue` (its `using` keyword in
      `cold.begin_loc`); a use is an `UnresolvedLookupExpr` that finds
      it, without ADL (clang's dump). The instantiation redoes the
      declarator (`subst/stmts.rs` `local_using_decl`), so a
      non-enumerator target is diagnosed then, with its note at the
      `using`, and a use (`local_using_ref`) is looked up again in the
      instantiated block. The uses of a declarator that failed
      (`Substituter::failed_usings`) are errors without another
      diagnostic (clang's invalid `UsingDecl`; WP-197).
    - A qualifier that is still dependent after substitution (a
      generic lambda in a function template specialization, or a
      local class in one) leaves the declarator unresolved, rebuilt
      with the substituted qualifier, until the inner specialization
      (`still_dependent_class_using`,
      `still_dependent_using_typename`).
- **Conflicts** ([namespace.udecl]/11, [basic.scope.scope]/4), as in
  clang's `CheckUsingShadowDecl`. Two distinct entities of one name in
  one scope:
  - an unresolved `using_if_exists` target conflicts with nothing;
  - a tag conflicts only with a tag;
  - two typedefs conflict unless they name the same type;
  - two functions conflict iff their types are the same ignoring
    exception specifications (else they overload);
  - anything else conflicts.
  - A new shadow that conflicts reports `target of using declaration
    conflicts with declaration already in scope`, with notes at the
    target and the conflicting declaration; no shadow is made.
  - A later declaration that conflicts with a shadow
    (`check_using_shadow_conflict`, from variable, function, typedef
    and tag declarations, including operator, literal-operator and
    conversion function names, WP-146) reports `declaration conflicts with
    target of using declaration already in scope`, and is marked invalid;
    in a class a member function instead hides the shadow
    ([namespace.udecl]/14, `Decls::hidden_shadow`).
  - Repeating a using-declaration of the same entity is allowed; the new
    shadow redeclares the old one.
- **Class scope:** kiln has no base classes yet (M3), so every
  using-declarator in a class is ill-formed, and Sema reports clang's
  diagnostic for the qualifier: `using declaration in class refers into
  'N::', which is not a class`, `using declaration refers to its own
  class`, or `using declaration refers into 'C::', which is not a base
  class of 'D'`. A declarator qualified by an
  enumeration works at class scope as anywhere else.
- **`using enum`** ([enum.udecl]): `act_on_using_enum(qual, class, name,
  name_loc)` takes the parser's classification of the (possibly
  qualified) name.
  - Not a type: `unknown type name E`; a type but not an enumeration:
    `'X' is not an enumerated type`. A missing name is a parse error:
    `using enum requires an enum or typedef name`.
  - An incomplete enumeration ([enum.udecl]/2): `enumeration 'E' is
    incomplete`, with a note; the `UsingEnum` declaration is made and marked invalid.
  - Otherwise a `UsingEnum` declaration names the definition, and each
    enumerator gets a `UsingShadow`, with the conflict checks above.
- **Namespace aliases** ([namespace.alias]):
  `act_on_namespace_alias(name, name_loc, qual, target, target_loc)`.
  - The target is looked up with `LookupKind::Nested` and must be a
    namespace or alias: `expected namespace name`.
  - Redeclaring an alias to the same namespace is allowed (it links to
    the previous one); to a different namespace: `redefinition of 'Q' as
    an alias for a different namespace`, with a note; any other earlier
    declaration of the name: `redefinition of 'Q'` (a namespace) or
    `redefinition of 'Q' as different kind of symbol`.
  - The alias is entered in lookup; at block scope, in `locals`.
- **Nested-name-specifiers:** `nns_global` and
  `nns_component(prefix, resolved, spelled)` build the `NnsId` the
  `Using` declaration records, keeping an alias as written
  (`Nns::NamespaceAlias`). `Sema::nns_str` prints one as written.
- **Uses:** a `DeclRefExpr` found through a shadow records it as its
  found declaration (`build_decl_ref_via`; `DeclRefExtra::found_decl`).

### 4.9 Structured bindings (`struct_bind.rs`)

**Landed (WP-122).** [dcl.struct.bind] for simple-declarations,
conditions, range-for loop variables, namespace scope (`static`,
`thread_local` and plain) and block scope, matching clang 19's AST,
diagnostics and IR.

- **Representation** (spec/ast.md §4). The decomposed object `e` is a
  `DeclKind::Var` with an empty name, located at `[` (clang's
  `DecompositionDecl`). Each name is a `DeclKind::Binding` declared
  before it. The links are write-once cold maps:
  - `set_decomp_bindings(v, list, bindings)` records the bindings, read
    back with `decomp_bindings(v)`, `is_decomposition(v)` and
    `binding_decomp(b)`;
  - `set_binding(b, expr, holding)` records the binding expression
    (`binding_expr(b)`) and, for a tuple-like type, the holding
    variable (`binding_var(b)`, built with `builder.holding_var`).

  `decl_name(v)` prints `[a, b]`, as clang's `printName` does.
- **Declaring.** The parser hands a declarator with a decomposition to
  `act_on_decomposition_declarator`:
  - In a template-declaration it is `decomposition declaration template
    not supported`.
  - In a condition (`DeclaratorContext::Condition`, not an
    init-statement) it warns `ISO C++17 does not permit structured
    binding declaration in a condition` (`-Wbinding-in-condition`) at
    `[`. Clang 19 does not implement P0963 yet and warns in C++26 too.
  - Only `static`, `thread_local`, `auto` and cv are allowed
    ([dcl.pre]/6). Another storage class, `constexpr`, `consteval`,
    `constinit` or `inline` is `decomposition declaration cannot be
    declared 'constexpr'`, or `... with 'static constexpr' specifiers`
    when there are several, at the first one. The declaration continues:
    a `constexpr` is still applied, which makes `e` const, as in clang.
  - A declarator chunk other than `&`/`&&` is `decomposition declaration
    cannot be declared with parentheses` or `... with type 'T'; declared
    type must be 'auto' or reference to 'auto'`.
  - A constrained placeholder is `decomposition declaration cannot be
    declared with constrained 'auto'`.

  `declare_decomposition` then declares the bindings: redefinitions are
  checked against earlier names, binding attributes are attached, and a
  pack `...a` (P1061) is reported as `a structured binding pack` is
  unsupported. It then declares `e`. A second declarator in the same
  group is `decomposition declaration must be the only declaration in
  its group` (`act_on_decomposition_not_alone`). A missing initializer
  is `decomposition declaration '[a, b]' requires an initializer`
  (`decomposition_requires_init`).
- **Initializing** (`act_on_init`):
  - `auto [a, b] = arr;` with a plain `cv auto` and an array initializer
    deduces `cv A`. It initializes element-wise with an
    `ArrayInitLoopExpr` (`decomp_array_type`, `init_decomposition_array`,
    clang's `TryArrayCopy`).
  - Otherwise `e` is deduced and initialized as an ordinary variable.

  `complete_decomposition` (clang's
  `CheckCompleteDecompositionDeclaration`) then gives every binding a
  type and an expression. An invalid `e` or an error initializer
  invalidates the bindings silently. A dependent `e` gives them
  `QualType::DEPENDENT` until instantiation.
  - **Arrays** (`decompose_array`): `e[i]` (an `ArraySubscriptExpr` on a
    `DeclRefExpr` to `e`), of the element type with `e`'s cv.
  - **Tuple-like** (`tuple_like`, `decompose_tuple`): applies when
    `std::tuple_size<E>` is a complete class.
    - A `value` that is not an integral constant expression is `cannot
      decompose this type; 'std::tuple_size<T>::value' is not a valid
      integral constant expression`.
    - Each binding gets a holding variable of type `tuple_element<i,
      E>::type&` (lvalue `e`) or `&&`, with the binding's name. It is
      initialized with `e.get<i>()` when a member template `get` has a
      non-type first parameter, else with an ADL-only `get<i>(xvalue
      e)`, which prints `adl`.
    - The binding's type is `tuple_element<i, E>::type`, as written,
      and its expression a `DeclRefExpr` to the holding variable.
    - `tuple_element` is checked after the `get` call. An incomplete
      specialization gets clang's `implicit instantiation of undefined
      template 'std::tuple_element<0, T>'` (`std_trait_ty(..,
      complain = true)`). A specialization without `type` is `cannot
      decompose this type; 'std::tuple_element<0>::type' does not name a
      type`.
    - A failed `get` call that had candidates is recovered as clang does
      (a `RecoveryExpr`), and the next binding is still checked.
      An empty `get` lookup (`use of undeclared identifier 'get'`) or a
      failed initialization stops at that binding.
    - The holding variable's initialization runs in a
      `FrameKind::BindingInit` frame, whose note is `in implicit
      initialization of binding declaration 'a'`.
  - **Classes** (`decomposable_class`, `decompose_members`): the bindings
    are `e.mi` (a `MemberExpr`) for the non-static data members of the
    one class in the hierarchy that has any. The member's type gets
    `e`'s cv, with `mutable` dropping const.
    - Fields spread over several classes are `cannot decompose class type
      'D': both it and its base class 'B' have non-static data members`
      (or `its base classes 'A' and 'B'`).
    - An ambiguous base is `cannot decompose members of ambiguous base
      class 'B' of 'D'`.
    - A lambda closure is `cannot decompose lambda closure type`.
    - An anonymous struct or union member is `cannot decompose class
      type 'S' because it has an anonymous struct|union member`, with a
      note.
    - An inaccessible member is `cannot decompose private|protected
      member 'x' of 'S'` at the binding, through the usual access
      check.
    - A union or a non-class type is `cannot decompose union|non-class,
      non-array type 'T'`.
    - A bit-field member makes the binding a bit-field lvalue. Every
      `DeclRefExpr` to it is marked `bitfield` (`mark_bitfield`, clang's
      `refersToBitField`), so `int &r = b;` is `non-const reference
      cannot bind to bit-field 'b'`.
  - A binding count that differs from the element count is `type 'T'
    decomposes into N elements, but M names were provided` (`1 name
    was`, `only 1 name was`).
- **Uses.** A `DeclRefExpr` to a binding is an lvalue of the binding's
  type, kind `Binding`. A binding is captured by a lambda as the entity
  it names ([expr.prim.lambda.capture]/12, P1091R3): `has_local_storage`
  and `may_need_capture` follow `binding_decomp`. Codegen loads a
  captured binding through the closure field. Reference binding
  (`conv::bind_reference`) looks through a binding to its expression for
  the bit-field check.
- **Templates.** `decomp_written` records each pattern's declared type.
  `templ/subst/decls.rs` re-declares the bindings and `e` from it, then
  completes the decomposition. A range-for loop variable and a
  condition are re-declared in `templ/subst/stmts.rs`. Binding
  attributes are not instantiated.
- **Constant evaluation and codegen.** The Oven and codegen treat `e`
  as an ordinary variable, followed by the holding variables
  (`kiln_oven` `holding_vars`, codegen `emit_var_decl`). A use of a
  binding evaluates or emits its binding expression. A namespace-scope
  `e` mangles as `_ZDC1a1bE` (`kiln_abi` `mangle_decomp.rs`, `DC
  <source-name>+ E`, never `L`-prefixed). A `constexpr` decomposition is
  rejected, so constant bindings come from a `const` `e` with a
  constant initializer, which is usable in constant expressions.
- **Fixed on the way.**
  - A reference that cannot bind (to `void`, or to a non-class type
    with no conversion) is `reference to type 'int' could not bind to
    an rvalue|lvalue of type 'void'` (`sema_reference_bind_failed`, the
    referee as written, clang's `FK_ReferenceInitFailed`).
  - `implicit instantiation of undefined template` names the
    specialization fully qualified (`Sema::decl_ty_str`, clang's
    printing of `getTypeDeclType`).
- **Left.**
  - Structured binding packs (P1061).
  - `constexpr` structured bindings (P2686; clang 19 rejects them too).
  - Instantiating binding attributes.
  - The NoOp `'const M'` cast clang adds when a const class `e` is
    copy-initialized from a prvalue.
  - Clang 19 rejects a capture of a binding by a lambda in a constant
    expression, and kiln accepts it.

## 5. Expressions (`expr.rs`, `conv/`)
- **Typed nodes:** each `act_on_*` builds clang's typed node, with
  implicit conversions spelled out as `ImplicitCastExpr`
  (sema-conv.md).
- **Errors:** an invalid operand produces an error expression
  (`act_on_error_expr`, tested with `is_error_expr`). Its consumers
  stay silent, so one mistake gives one diagnostic.
- **Supported expressions:**
  - **names:** DeclRefExpr, enumerators, functions with
    function-to-pointer decay
  - **literals:** via `kiln_sema::literal`; user-defined literals
    call literal operators (sema-overload.md §6.7); when no form
    matches, clang's `no matching literal operator` is reported
  - **operators:**
    - unary and binary operators, including compound assignment (its
      computation type promotes the left operand's type, bit-fields
      by width, and an unscoped-enumeration left operand is then an
      error: sema-conv.md §6)
    - `?:`, the comma operator
    - increment and decrement (of `bool`, or `_Atomic(bool)`: `++` is
      clang's DefaultError `-Wincrement-bool` extension and is then
      accepted, `--` is always an error; WP-165)
  - **calls:** argument conversion, arity notes, variadic default
    promotions
  - **member access:** `.` and `->` on records and through
    `IndirectField`; members found in base classes, static members,
    enumerators and member functions, implicit `this->`, `this`,
    `.*`/`->*` and `&C::m`, all access-checked (sema-class.md §2.4–§2.6,
    WP-066b)
  - **member calls:** `CXXMemberCallExpr` for a single candidate
    (sema-class.md §8.3, "Landed (WP-066b)"); overloaded member
    functions wait for WP-068a
  - **subscripts:** either operand order
  - **`sizeof`/`alignof`:** of types and expressions, using
    `kiln_abi` layout
  - **casts:** C-style, functional, `static_cast`,
    `reinterpret_cast`, `const_cast`, with
    `mark_part_of_explicit_cast`. To a reference (`expr/ref_cast.rs`,
    WP-207): `const_cast<T&&>` (and the C-style cast's const step) of a
    class prvalue of the same unqualified class materializes it
    (`MaterializeTemporaryExpr`, [expr.const.cast]/3) and is a `NoOp`
    xvalue; `reinterpret_cast` (and the C-style cast's reinterpret step)
    binds any glvalue, an xvalue as an lvalue reference too (`NoOp` for
    the same type, else `LValueBitCast`); `const_cast` of a non-lvalue to
    `T&`, of a non-class prvalue to `T&&`, and `reinterpret_cast` of a
    prvalue are `{const_cast|reinterpret_cast} from rvalue to reference
    type 'T &'` (clang `err_bad_cxx_cast_rvalue`). A failed cast's
    `RecoveryExpr` has the referenced type and the reference's value
    category, as every `RecoveryExpr` of a reference type does (clang
    `CreateRecoveryExpr`), so it is not diagnosed again. Not yet: the
    C-style cast's own rvalue message (`(S&)mk()` reports the
    initialization failure of its static step instead of `C-style cast
    from rvalue to reference type`).
- **Constant-folding warnings:** division by zero, negative shifts and
  over-wide shifts warn when the operands fold. They are run-time
  behavior diagnostics anchored at the right operand (clang's
  `DiagRuntimeBehavior` → `DiagIfReachable`, §5.12): they are dropped
  where `runtime_diag_quiet` holds, deferred to the end of a function
  body (and dropped there when unreachable), and skipped in the
  initializer of a namespace-scope `constexpr` variable or of a static
  data member's first declaration (WP-235).
- **Pointer arithmetic on `void` or a function type** ([expr.add]/1;
  `check_pointer_arith`, WP-219): `+`, `-`, `++`, `--` and compound
  assignment on a pointer to `void` or to a function type is an error in
  C++ (`arithmetic on a pointer to void`, `arithmetic on pointers to the
  function type 'void ()'` for two pointer operands), not the GNU C
  extension (`-Wgnu-pointer-arith`, kept for C). A pointer difference
  (`check_pointer_pair_arith`, clang `checkArithmeticBinOpPointerOperands`,
  WP-230) runs after `'T *' and 'U *' are not pointers to compatible
  types` too: a `void` pointee on one side is diagnosed for that side,
  on both as `pointers to void`; likewise a function pointee, the
  two-pointer form naming both pointees when the pointer types differ
  (`arithmetic on pointers to function types 'void ()' and 'void
  (int)'`); otherwise an incomplete pointee, left side first.
- **Member-pointer equality** ([expr.eq]/4, clang's
  `CheckCompareOperands`, WP-085): when either operand of a comparison
  is a member pointer:
  - `<`, `>`, `<=`, `>=` (and `<=>`, not yet distinguished) are
    `invalid operands to binary expression ('int B::*' and 'int B::*')`;
  - a null pointer constant on either side (`0`, `nullptr`, checked
    right operand first) converts to the other operand's type
    (`NullToMemberPointer`); the result is `bool`;
  - two member pointers of the same type compare directly; otherwise
    the right operand converts to the left's type, then the left to the
    right's, by a standard conversion (`BaseToDerivedMemberPointer` from
    the base's to the derived class's member pointer, plus
    qualification conversions). A conversion through a virtual base
    reports its conversion error at the operand, followed by the
    distinct-pointer error below;
  - no conversion either way: `comparison of distinct pointer types
    ('int B::*' and 'int O::*')` when both are pointers or member
    pointers, otherwise (a non-null integer, `nullptr` as a non-null
    operand of a different kind) `invalid operands`.
  Not yet: the composite type that needs a cv-union of both sides
  (`const int B::*` vs `volatile int D::*`; [expr.type]/4.6) and
  constant evaluation of the comparison (WP-076).
- **Composite pointer type** ([expr.type]/4; `composite_pointer` in
  `expr.rs`, used by `==`, `<`, `<=>` and `?:`): after the null,
  same-type and one-way-conversion cases, `cv_combined_pointer` tries a
  pointer to either side's unqualified pointee with the cv-qualifiers of
  both (`void *` vs `const B *` is `const void *`; `B *` vs `const D *`
  is `const B *`), taking the first that both operands reach by a
  standard conversion (WP-155). Not yet: multi-level cv-combination
  (`int **` vs `const int **` is `const int *const *`) and the member
  pointer case above.

### 5.1 Unevaluated operands
`unevaluated > 0` inside `sizeof`, `alignof` and `noexcept` operands.
Names there are not odr-used.

### 5.2 Odr-use
- A DeclRefExpr to a variable is a *potential* odr-use, queued in
  `pending_odr`.
- `act_on_full_expr` resolves the queue at the end of each
  full-expression ([basic.def.odr]/5, [intro.execution]/5):
  - It walks the expression and collects the potential results of
    every lvalue-to-rvalue conversion.
  - A variable usable in constant expressions that is named there is
    marked `non_odr_use_constant`. So is a reference usable in constant
    expressions.
  - Every other queued variable is marked `used`.
- Names inside unevaluated operands are marked
  `non_odr_use_unevaluated`.
- The dump reads `used` and `referenced` from the first declaration.

### 5.3 The `noexcept` operator (`except.rs`)
`act_on_noexcept_expr(e, loc, rparen)` ([expr.unary.noexcept]) builds
a `CXXNoexceptExpr`: a `bool` prvalue whose operand is unevaluated
(the parser brackets it with `enter_unevaluated`/`exit_unevaluated`).
Its value is fixed at construction, `!can_throw(e)`, and stored on the
node (`ExprStore::noexcept_value`); the Oven and codegen read it as a
constant and never look at the operand.

`can_throw(e)` is true when `e` is potentially-throwing
([except.spec]/6), walking the operand and every subexpression:

| construct | implemented |
|---|---|
| a call whose callee's function type (through a pointer or reference) is not non-throwing; a callee without a prototype throws | yes |
| `throw` expression; `new` expression | yes (neither is produced by the M2 parser) |
| `dynamic_cast` to a reference type needing a run-time check | yes (not produced by the M2 parser) |
| `typeid` of a glvalue of polymorphic class type | M3 (no polymorphic classes in M2) |
| implicit calls: constructors, destructors, conversion functions, allocation functions | yes: constructors (`CXXConstructExpr`), destructors (`CXXBindTemporaryExpr`) and conversion functions (implicit member calls), each resolving the callee's spec first (WP-112); every `new` counts as throwing |
| `__attribute__((nothrow))` on the callee | M3 (not parsed) |
| a dependent operand (value-dependent `CXXNoexceptExpr`) | yes (lazily instantiated member specs, §4.7) |

The operands of nested `sizeof`, `alignof` and `noexcept` are not
looked at (they are unevaluated). An erroneous operand gives an error
expression.

### 5.4 Builtin functions (`builtins.rs`)
clang's `LazilyCreateBuiltin`. kiln knows one builtin,
`bool __builtin_is_constant_evaluated() noexcept` (id 722, clang's
`BIis_constant_evaluated`; `kiln_ast::decl::builtin`).
- **Declaration:** when an unqualified id-expression's lookup finds
  nothing and the name is a builtin (`lazily_create_builtin`), Sema
  declares it in the translation unit: an implicit `LinkageSpecDecl`
  (`extern "C"`) holding an implicit `extern` `FunctionDecl` with the
  builtin's function type, its id in the `builtin_id` cold data, an
  implicit `Builtin` attribute (invalid location, `a` = id) and an
  implicit `NoThrow` attribute. Later lookups find that declaration.
- **Name:** a `DeclRefExpr` to a builtin is a prvalue of the
  placeholder type `QualType::BUILTIN_FN` (`<builtin fn type>`).
- **Calls:** `act_on_call` converts the callee with
  `BuiltinFnToFnPtr` (a pointer to the builtin's function type) and
  type-checks the call as an ordinary one.
- **Any other use** is `builtin functions must be directly called`
  (`check_builtin_fn_use`), reported at the name by
  `implicit_convert` and `decay` (initialization, operands,
  arguments), by unary `&`, and by an expression statement (which then
  gets no unused-result warning). The expression recovers as an error
  expression.
- **Evaluation:** the Oven answers `true` in a manifestly
  constant-evaluated context and `false` otherwise (spec/oven.md);
  codegen emits `i1 false` (spec/codegen.md).
- **Library functions** (WP-171, clang `ActOnFunctionDeclarator`'s
  library-builtin recognition + `AddKnownFunctionAttributes`,
  `add_known_function_attrs`): the first declaration of a function
  named in `builtin::LIB` (clang 19's library builtins carrying
  `NoThrow` / `Const` / `Pure`: `<string.h>`, `<stdio.h>` `*printf`,
  `<ctype.h>`, `<wchar.h>`, `<math.h>` d/f/l), declared in a namespace
  scope context (TU or namespace, through linkage specs) with C
  language linkage and with the row's type ignoring the exception
  specification, gets implicit `NoThrow` and `Const`/`Pure` attributes
  (after its written ones). Sig letter `a` is `__builtin_va_list`
  (decayed when an array), `P` the type of the TU-scope `FILE`
  typedef (none yet: not recognized). Redeclarations inherit them
  (`Ast::has_nothrow_attr` reads the chain). A `NoThrow` callee makes
  a call non-throwing (`can_throw`, so `noexcept(strlen(s))` is
  true). No `Builtin` attribute is attached, so the call stays an
  ordinary call: clang's lowering of recognized `memcpy`/`fabs`/`abs`
  calls to intrinsics and its implicit `format` / `alloc_size`
  attributes (`-Wformat*`) are not modelled.
### 5.5 `throw` (`except.rs`)
`act_on_throw(operand, loc)` ([expr.throw]) builds a `void`
`CXXThrowExpr`:
- With `-fno-exceptions` it is an error
  (`cannot use 'throw' with exceptions disabled`).
- The exception object's type is the operand's, with arrays and
  functions decayed and top-level cv removed; it must not be (a
  pointer to) an incomplete type other than cv `void` (clang's
  `CheckCXXThrowOperand`).
- The operand is copy-initialized into it after the implicit move
  ([class.copy.elision]/3), which applies only when the named local's
  scope does not extend past the innermost enclosing try-block
  (`thrown_var_in_scope`, WP-100).
- In `?:`, a throw operand gives the other operand's type and value
  category ([expr.cond]/2).
- clang's exemption for system headers is not implemented. The Oven
  treats a `throw` it reaches as not a constant expression; codegen
  lowers it (spec/codegen.md §7b).

### 5.6 `typeid` (`rtti.rs`)
`act_on_typeid(Ok(expr) | Err(type), loc, rparen)` ([expr.typeid]):
an lvalue of type `const std::type_info`.
- `std::type_info` must have been declared (clang's
  `you need to include <typeinfo> before using the 'typeid' operator`),
  and RTTI enabled.
- References and top-level cv are ignored; a cv-qualified expression
  operand gets a `NoOp` cast to its unqualified type, as in clang.
- A class type must be complete.
- A glvalue of polymorphic class type is potentially evaluated
  (`typeid_expr`, sema-class.md §5.1); other operands are unevaluated.
- `-Wpotentially-evaluated-expression` (WP-219, `side_effects.rs`
  `check_typeid_side_effects`; in `-Wunevaluated-expression`): a
  potentially evaluated operand with possible side effects (as for
  `-Wunevaluated-expression`, but calls count) warns `expression with
  side effects will be evaluated despite being used as an operand to
  'typeid'` at clang's expression location. Not in instantiations, not
  for instantiation-dependent operands.

### 5.7 `dynamic_cast`
Parsed; `act_on_cast` reports `dynamic_cast` as not supported
yet (it needs polymorphic classes).

### 5.8 GNU extensions (`gnu.rs`)
Kiln accepts these GNU extensions in C++ and matches clang's behaviour
for them.

- **Statement expressions** `({ ... })`:
  - `check_stmt_expr_allowed` rejects them outside a function body
    (`statement expression not allowed at file scope`).
  - `act_on_start_stmt_expr` sets aside the enclosing
    full-expression's pending odr-uses (§5.2), and `act_on_stmt_expr`
    restores them.
  - The final expression statement goes through
    `act_on_stmt_expr_result`: decay, then copy-initialization of a
    prvalue of its cv-unqualified type.
  - The type is that of the last non-null statement, looking through
    labels, when that is an expression statement; otherwise `void`.
    The result is always a prvalue.
  - An unused statement expression warns as clang's
    `isUnusedResultAWarning` does.
- **Unused results** (`unused_warning`, clang `isUnusedResultAWarning`
  / `DiagnoseUnusedExprResult`; WP-158): a functional, C-style or
  `static_cast` constructor conversion (`X(1);`) is checked through its
  operand, looking through temporary binding, materialization and
  cleanups; a constructor call warns only when the constructor or its
  class is `[[nodiscard]]` (`ignoring temporary created by a constructor
  declared with 'nodiscard' attribute[: msg]`, -Wunused-value). A braced
  or scalar functional cast warns `expression result unused`. A C-style
  cast written `(void*)` (exactly, not through a typedef) warns
  `expression result unused; should this cast be to 'void'?`
  (-Wunused-value, clang `warn_unused_voidptr`) with only the fix-it
  removing the `*` as its range (WP-203).
  - `emit_unused` (WP-182) follows clang's order: a comparison is
    `-Wunused-comparison` only when it is the unused expression itself
    (`top_comparison`: not parenthesized, not a comma's right operand or
    a conditional's arm, which are `expression result unused`); an `==`
    / `!=` (built-in or overloaded) whose left operand is an lvalue (not
    a materialized temporary) adds the note `use '=' to turn this
    equality comparison into an assignment` / `use '|=' to turn this
    inequality comparison into an or-assignment`. A call to a `pure` or
    `const` function with a non-`void` result warns `ignoring return
    value of function declared with pure|const attribute` [-Wunused-value]
    at the call's start. Nothing but `nodiscard` is diagnosed for an
    expression spelled in a macro body (clang `ShouldSuppress`).
  - **In an instantiation (WP-217)**: clang rebuilds every expression
    statement and for-increment (`TransformExprStmt` → `ActOnExprStmt`),
    so the warning of a non-dependent one is reported again in each
    instantiation, with its notes: `emit_unused` marks it
    `allow_duplicate` (spec/diag.md §7.4) inside an instantiation. Not
    the comma's left operand (checked once, as the comma is built). A
    deprecated use is not re-diagnosed (the shared node is not rebuilt).
    The warning's range is the whole expression where clang's is the
    callee (a caret difference only).
  - **Comma (WP-182)**: the left operand of a built-in comma is checked
    the same way when the comma is built (clang `CheckCommaOperands`),
    with the generic warning spelled `left operand of comma operator has
    no effect` [-Wunused-value] (not in a SFINAE context). Nothing is
    checked in an unevaluated operand or for a type-dependent operand
    (it is checked on instantiation, with the instantiation note), and
    an `[[assume(...)]]` argument is checked like any other expression.
  - **Reachability (WP-203)**: the generic warnings (`expression result
    unused`, `left operand of comma operator has no effect`) go through
    `diag_if_reachable` (clang `DiagIfReachable`); the comparison,
    `nodiscard`, `pure`/`const` and `(void*)` warnings are emitted at
    once. In a function body the warning is queued on
    `FnState::runtime_diags` with its expression as anchor; at the end
    of the body, when no error occurred and outside a template, those
    whose anchor the reachability walk (`unreachable_exprs`, the
    structured CFG mirror of §7.5) meets only on dead paths are dropped
    (`return; x;`, `if (0) x;`, the unmatched case of a constant
    switch, the pruned arm of `true ? a : (x, 1)`); a template's are
    emitted unfiltered and an instantiation filters its own; an `if
    consteval` in the body (no CFG) keeps them all. Outside a function
    the warning is not emitted in the initializer of a `constexpr`
    variable or of a static data member's first, non-inline declaration
    (clang's `ManglingContextDecl` test, read from
    `lambda_mangle.var`); otherwise at once.
  - The warning carries the expression's range, so a macro-argument
    location gets clang's `expanded from macro` note (spec/diag.md §8.4).
  - The run-time diagnostics of §5.15 use the same queue and are
    dropped in the same contexts (§5.12, `runtime_diag_quiet`).
  - In a default argument, the parser reports
    `default argument may not use a GNU statement expression`.
- **Unused variables (WP-203; clang `ShouldDiagnoseUnusedDecl` /
  `DiagnoseUnusedDecl`)**: `unused variable 'x'` [-Wunused-variable, in
  -Wunused and so -Wall; off by default] at the variable's location.
  - When a block scope is popped (`pop_scope`; the function-body scope
    from `end_function_body`, before the queued runtime warnings, as
    clang leaves the body scope before `ActOnFinishFunctionBody`), its
    unreferenced local variables are diagnosed sorted by location
    (`unused_var.rs`, `diagnose_unused_locals`), unless an error was
    reported since the scope was entered (`Scope::errors`, clang
    `hasUnrecoverableErrorOccurred`) or the scope belongs to an
    instantiation.
  - Not diagnosed: an invalid or unnamed variable; one referenced or
    used (`sizeof(x)` counts); one marked `[[maybe_unused]]` /
    `__attribute__((unused))`, or whose typedef or class is; a `_`
    placeholder with automatic storage; a variable outside a function,
    `extern` or a handler's exception variable; one of dependent or
    incomplete type; one whose (array element) class has a non-trivial
    destructor, or is initialized by a non-trivial constructor call that
    is not a constant initialization, or by a type-dependent
    initializer. A reference lifetime-extending a temporary is judged by
    the temporary's type.
  - A variable of dependent type is diagnosed as its instantiation is
    created (`diagnose_unused_inst_var`, from `subst` `var_decl`, clang
    `TemplateDeclInstantiator::VisitVarDecl`) by the pattern's
    referenced flag, with the instantiation note.
  - Structured bindings (WP-217): a structured binding declaration is
    diagnosed at its `[`, named `[a, b]`, when none of its bindings is
    referenced or marked unused (and not all are placeholders), whatever
    its storage or reference kind; one of dependent type is diagnosed as
    it is instantiated.
  - A class-typed variable with a type-dependent initializer (WP-217) is
    diagnosed in the template definition unless its class declares a
    non-trivial constructor or the initializer is an unresolved
    construction with dependent arguments.
  - `-Wunused-exception-parameter` (WP-217, clang
    `warn_unused_exception_param`, off by default): a named exception
    variable never referenced, when its handler scope is left; not a `_`,
    `[[maybe_unused]]` (the parser takes attributes on the handler's
    declaration) or one whose class has a non-trivial destructor (a
    reference to one still counts).
  - `-Wunused-but-set-variable` (WP-217, clang `DiagnoseUnusedButSetDecl`
    / `RefsMinusAssignments`, in -Wunused): `variable 'x' set but not
    used` for a local whose every reference is the target of a
    discarded-value assignment, compound assignment or increment (an
    expression statement, a comma's left operand, a for-increment, an
    operand cast to void). `Sema::refs_minus_assign` counts references
    minus such targets. Not for a reference, class, dependent,
    `[[maybe_unused]]` or `_` variable, a parenthesized assignment, or a
    compound assignment/increment of a volatile one; diagnosed with the
    unused variables in location order.
  - Namespace-scope variables (WP-217, `unused_file_scope.rs`, clang
    `ShouldWarnIfUnusedFileScopedDecl`): at the end of the translation
    unit, a variable with internal linkage declared in the main file and
    never referenced is `unused variable 'x'` [-Wunused-variable], or
    `unused variable 'x'` [-Wunused-const-variable, in -Wunused-variable]
    when `const`, in declaration order and none once an error occurred;
    not a variable template, a static data member, or one referenced but
    not odr-used.
  - Parameters (WP-234): a parameter that is only assigned is
    `parameter 'p' set but not used` [-Wunused-but-set-parameter, in
    -Wextra], by the same rule as a variable (never a `_` placeholder).
    The parameters' warnings come first, in location order, then the
    body's (clang reports them in `ActOnFinishFunctionBody` before the
    body scope is popped).
  - Unneeded file-scope variables (WP-234, clang
    `ShouldRemoveFromUnused`): an internal variable that is referenced
    but not odr-used is `variable 'x' is not needed and will not be
    emitted` [-Wunneeded-internal-declaration, in -Wunused-function and
    -Wunused-template]; an unused variable template is `unused variable
    template 'x'` [-Wunused-template]. Static data members of classes
    with internal linkage, and out-of-line explicit specializations of
    class template members, are diagnosed at their definition; inline
    variables in the main file are diagnosed as clang does.
  - Gaps: a variable named only in a default argument counts as
    odr-used, so it is not reported as unneeded; C++26 `_` placeholder
    redeclarations ("redefinition of '_'").
- **`__builtin_offsetof(T, designator)`** (`act_on_builtin_offsetof`
  with `OffsetofComponent`s; clang's `BuildBuiltinOffsetOf`): a
  `size_t` prvalue `OffsetOfExpr`.
  - The byte offset is computed from `kiln_abi` layout.
  - `T`, and every class it steps into, must be a complete class
    type; a bit-field member is an error. Array indices must be
    integral constants; a non-constant index, and a result outside
    0 to 2^32 - 1 (which a negative index gives), are unsupported.
    clang's error for a member of a virtual base is not implemented.
  - For a class that is not standard-layout it warns
    `-Winvalid-offsetof`. Standard-layout is computed from members
    and bases (`is_standard_layout`) until the definition bits track
    it.
- **Binary conditional** `x ?: y` (`act_on_binary_conditional`, in
  `forms.rs`): a `BinaryConditionalOperator` with `x` shared through an
  `OpaqueValueExpr`. Unless both operands are glvalues of the same
  type, `x` gets the usual unary conversions first.
- **Compound literals** `( T ) { ... }` (`act_on_compound_literal`;
  clang's `BuildCompoundLiteralExpr`): a prvalue
  `CompoundLiteralExpr` of the written type, holding the
  list-initialization of cv-unqualified `T`, done as if by a C-style
  cast at the `(`. clang's diagnostics follow from that:
  - a reference type cannot bind to an initializer list;
  - more than one element for a scalar is a function-style cast
    error;
  - an incomplete type is diagnosed with the forward-declaration note.

  - a class literal (WP-191) is list-initialized as a direct-list
    initialization (`list_construct`, `InitStyle::DirectList`): a
    class with constructors holds a `CXXTemporaryObjectExpr`, an
    aggregate its `InitListExpr`; an abstract class is an error, and a
    class with a non-trivial destructor wraps the literal in a
    `CXXBindTemporaryExpr`;
  - a dependent literal, including an array literal whose element
    type or bound is dependent, holds the list as written (an
    `InitListExpr` of type `void`). Substitution rebuilds it through
    `act_on_compound_literal`, so every check above runs per
    instantiation (WP-235).
  - a literal at namespace scope (no enclosing function; this includes
    a default member initializer and a variable template's
    initializer) is [GNU] and must have a constant initializer (C99
    6.5.2.5p3; clang's `CheckForConstantInitializer`). Otherwise the
    error is "initializer element is not a compile-time constant",
    reported at the culprit (WP-235):
    - for an aggregate list, the first element, recursively, that does
      not fold (`try_fold`; implicit value-initializations are
      constant). The location is that of clang's `getExprLoc`: the
      operator of a binary or member expression, otherwise the first
      token;
    - for a constructor call, the type-id after the `(`;
    - for a scalar or vector literal, its `{`.

    A dependent literal is checked when instantiated. A valid
    non-array literal is a prvalue, as it is inside a function, and its
    variable is constant-initialized.

  Deviations:
  - Array literals at namespace scope are reported as not supported
    yet. clang makes them lvalues naming an internal
    `.compoundliteral` global, which needs a static-storage object in
    the Oven and codegen.
  - kiln does not wrap the elements of a namespace-scope literal in
    `ConstantExpr` nodes, as clang does.
- **Flexible array members** (`flex_array.rs`, WP-271; a C99 feature,
  clang `ActOnFields` / `CheckFlexibleArrayInit`). A non-static data
  member of array-of-unknown-bound type `T[]` is accepted when its
  element type is complete (`field_complete_type`; an incomplete or
  abstract element is diagnosed as for a bounded array). The member
  keeps the incomplete type; the class is complete and records
  `DefinitionBits::HAS_FLEXIBLE_ARRAY_MEMBER`. Layout: size zero at the
  aligned end of the preceding members, counting for the alignment
  (`sizeof(struct { int n; char d[]; }) == 4`); LLVM type `[0 x T]`;
  implicit copies copy only the other members. At the `}`
  (`check_flexible_array_members`, run per instantiation; each
  diagnostic is `per_instantiation`, spec/diag.md §7.4), over the
  fields in order:
  - not the last field of a struct/class: error `flexible array member
    'd' with type 'T[]' is not at the end of struct`, note `next field
    declaration is here` at the next field; field and class invalid;
  - in a union: `-Wgnu-flexible-array-union-member`; otherwise, when no
    named field precedes it: `-Wgnu-empty-struct`;
  - a class with a virtual base (direct or inherited): error `...not
    allowed in struct which has a virtual base class`;
  - always `flexible array members are a C99 feature` [-Wc99-extensions];
  - a non-dependent element type with non-trivial destruction: error
    `flexible array member 'd' of type 'T[]' with non-trivial
    destruction`; field and class invalid;
  - a field whose class has a flexible array member gives this class one
    too; in a struct/class it warns `field 'a' with variable sized type
    'A' not at the end of a struct or class is a GNU extension`
    [-Wgnu-variable-sized-type-not-at-end, on by default] when not last,
    else `'a' may not be nested in a struct due to flexible array member`
    [-Wflexible-array-extensions];
  - an array of such a class: `'A' may not be used as an array element
    due to flexible array member` [-Wflexible-array-extensions] at the
    `[` (the declarator's array chunk).

  Except where noted, the warnings are Extensions (`-pedantic`). A
  default member initializer of a `d[]` member is the parse error `array
  bound cannot be deduced from a default member initializer` (at the `=`
  or `{`; the member is invalid and gets no checks). In an aggregate
  initialization the member's initializer is: an empty list, the GNU
  extension `flexible array initialization is a GNU extension`
  [-Wgnu-flexible-array-initializer] with the note `initialized flexible
  array member 'd' is here` and `zero size arrays are an extension`
  [-Wzero-length-array]; elements, an error `initialization of flexible
  array member is not allowed` (same note) unless they initialize a
  variable without automatic storage directly (`Sema::init_var` with no
  `init_entity`). The Oven rejects the decay of a member subobject of
  unknown bound (`array-to-pointer decay of array member without known
  bound is not supported`, clang `addUnsizedArray`).

  Deviation: elements initializing a static variable (clang's extension,
  which emits the variable with a larger type) are reported as not
  supported yet.
- **Zero-length arrays** `T[0]`: `zero size arrays are an extension`
  [-Wzero-length-array, in -Wgnu] at the bound, per instantiation; an
  error in a SFINAE context (above).
- **`__null`** (`gnu_null.rs`; `NULL` in the resource `<stddef.h>`): a
  `GNUNullExpr` prvalue of the signed integer type as wide as a data
  pointer (`int`, `long` or `long long`; `long` on LP64, clang's
  `ActOnGNUNullExpr`), with value 0. It is a null pointer constant
  (`conv::is_null_pointer_constant`), so it converts to any pointer,
  pointer to member and `std::nullptr_t`; as an integer it converts like
  any `long` (so `g(int)`/`g(void*)` is ambiguous). Constant-evaluates to
  0; mangles as the literal `Ll0E`; prints as `__null`. Warnings (both on
  by default, not runtime-behavior diagnostics):
  - `implicit conversion of NULL constant to 'T'` [-Wnull-conversion, in
    -Wconversion; alias -Wconversion-null] for an implicit conversion to
    a scalar type other than its own, a pointer, a pointer to member or
    `std::nullptr_t` (clang's `DiagnoseNullConversion`, from the
    implicit-conversion walk, `impcast.rs`). It is reported at `__null`,
    or at the `NULL` macro name when `__null` came from `NULL`'s
    expansion; after taking both locations out of macro arguments, it is
    dropped when that location and the conversion's context are in
    different files or macro expansions (`#define MYNULL __null` does
    not warn).
  - [-Wnull-arithmetic] (clang's `checkArithmeticNull`), at the operator
    of a built-in `+ - * / % << >> & ^ |`, compound assignment, or
    comparison with a `__null` operand (implicit casts and parentheses
    stripped), unless the other operand is a pointer to member or a
    function: `use of NULL in arithmetic operation` for the arithmetic
    operators; for a comparison, `comparison between NULL and non-pointer
    (NULL and 'T')` / `('T' and NULL)` when only one side is `__null`
    and the other is neither a pointer nor an array.
- **GNU `typeof`** (`gnu_typeof.rs`; parsing in spec/parse.md §4):
  `__typeof ( type-id )` is `TypeOf` sugar over that type
  (`act_on_typeof_type`), printed `typeof(T)`. `__typeof e` /
  `__typeof ( e )` (`act_on_typeof_expr`; clang
  `ActOnTypeofExpression`) is sugar over the unevaluated operand's
  expression type: an lvalue adds no reference (`__typeof(ri)` for
  `int& ri` is `int`), cv-qualifiers are kept, an id-expression or
  bit-field member gives its expression type, a function name its
  function type, and side effects are not diagnosed (unlike
  `decltype`). It prints `typeof (e)` (the parentheses are a
  `ParenExpr`). An erroneous operand or an unresolved overload set
  (`check_overload_use`: "reference to overloaded function could not
  be resolved; did you mean to call it?") makes the type specifier an
  error that combines with no other specifier. A type-dependent
  operand gives a dependent type whose canonical form is keyed on the
  operand's profile (clang `DependentTypeOfExprType`), rebuilt at
  substitution (`templ/subst/types.rs`) and mangled as clang's vendor
  type `u6typeof`; a dependent `__typeof(T)` is canonically `T`.
  `__typeof_unqual` is the same with the cv-qualifiers dropped
  (canonically, `ctx.unqualified`; `typeof_unqual (e)` prints).
  No diagnostics of its own (no pedantic warning, as clang).
  `tests/conformance/ext.gnu.typeof/`.

### 5.9 New- and delete-expressions (`new_delete.rs`, WP-061a)
clang's `ActOnCXXNew`/`BuildCXXNew` and `ActOnCXXDelete` for non-class
and aggregate element types.
- **Implicit global operators** ([basic.stc.dynamic.general]/2):
  `declare_global_new_delete` runs once, the first time a new- or
  delete-expression, a declaration of a global `operator new`/`delete`,
  or an unqualified or `::`-qualified id-expression naming one
  (`::operator new(n)`, WP-158) needs it (clang's
  `DeclareGlobalNewDelete`). It declares, as implicit
  TU-scope functions in clang's order, `operator new`/`new[]` taking
  `(size_t)` and `(size_t, std::align_val_t)`, and `operator
  delete`/`delete[]` taking `(void*)`, `(void*, align_val_t)`,
  `(void*, size_t)` and `(void*, size_t, align_val_t)`, all
  `noexcept`. `std::align_val_t` is the program's own declaration, else
  an implicit `enum class align_val_t : size_t` in an implicit
  namespace `std` (clang's `getStdAlignValT`). The nothrow forms exist
  only if the program (`<new>`) declares them.
- **User declarations** of a global `operator new`/`delete`
  (`act_on_global_new_delete_decl`) redeclare a matching implicit one; a
  deallocation function without an exception specification is
  implicitly `noexcept` ([except.spec]/11).
- **The allocated type** ([expr.new]/1-2): not a function type, a
  reference or an incomplete type; `auto` is deduced from a single
  parenthesized or braced argument (`new auto(x)`; `new auto*(p)` is
  unsupported). The new-declarator's first bound is converted to `size_t`
  ([expr.new]/8: integral or unscoped enumeration, or a class with one
  conversion, which is unsupported); a constant first bound that is
  negative, too large, or smaller than the number of initializers is an
  error; later bounds are converted constant expressions. A `new T[]`
  takes its bound from the initializer. In a parenthesized type-id a
  non-constant bound is clang's warning `when type is in parentheses,
  array cannot have dynamic size` (see the gaps below).
- **Allocation function** ([expr.new]/12-19): `PASS_ALIGN` when
  `alignof(T)` exceeds `__STDCPP_DEFAULT_NEW_ALIGNMENT__` (16 on both
  targets, `kiln_abi::Target::default_new_align`). Global lookup only
  (class-scope `operator new` is `todo(WP-068b)`): the candidates whose
  leading parameters are `size_t` (and `align_val_t` when aligned) and
  that accept the placement arguments, preferring the most exact
  matches; with `PASS_ALIGN` a second round without the alignment
  argument follows (clang's two-phase lookup). Failure reports clang's
  `no matching function for call to 'operator new'` with candidate notes,
  or for a placement `new (p) T` with no `(size_t, void*)` form, `no
  matching 'operator new' function for non-allocating placement new
  expression; include <new>`.
- **Deallocation function for new** ([expr.new]/28): for a placement
  form, the one with the same extra parameters; otherwise the sized usual
  one (with `align_val_t` when aligned).
- **Initializer** ([expr.new]/23): none is default-initialization (a
  `const` scalar or array is an error: `default initialization of an
  object of const type 'const int[]'`, the array type printed for array
  new); `()` is an `ImplicitValueInitExpr`; `(x)` copy-initializes a
  scalar (more than one argument is clang's `excess elements in scalar
  initializer`); `{...}` is list-initialization (§6) of
  `T` or of the array `T[n]`, with an `array_filler` for the tail.
  Class element types report "not supported yet" for default- and
  parenthesized initialization (they need `CXXConstructExpr`,
  WP-067b); braced aggregate initialization works.
- **Delete** ([expr.delete]): the operand is converted to a pointer
  prvalue (a class operand with a conversion function is unsupported).
  Errors and warnings follow clang: `cannot delete expression of type`,
  `cannot delete pointer to incomplete type 'void'` (an error for
  `void*` in C++26), the incomplete-type warning, and `'delete' applied
  to a pointer-to-array type ... treated as 'delete[]'` (then
  `ARRAY_FORM`). The deallocation function is the usual one: sized when
  the type is complete and the form is not array (no array cookies
  without destructors), aligned when over-aligned. Class element types
  with a non-trivial destructor are unsupported (WP-067b / WP-076);
  class-scope `operator delete` is `todo(WP-068b)`.
- The element class's destructor ([expr.delete]/6, and for `new T[n]`
  [expr.new]/27) is selected (`mark_element_dtor_referenced`, skipped
  for an invalid or dependent class): a deleted one (such as a union's
  with a non-trivial variant member, [class.union.general]/3) is
  `attempt to use a deleted function` with the deletion notes and the
  expression fails; an inaccessible one is `calling a private destructor
  of class 'P'` at the operand (`sema_access_dtor`). For `new T[n]`
  both are at the allocated type, the access error worded as for a
  temporary (`temporary of type 'P' has private destructor`), as clang's
  `ActOnCXXDelete` / `BuildCXXNew` do.
- Gaps: `-fno-sized-deallocation` only removes the macro (Sema always
  selects the sized form, clang's C++14+ default); `-Wmismatched-new-delete`
  is not reported; a non-constant bound in a parenthesized type-id is a
  plain warning (clang's is an extension warning, so an error under
  `-pedantic-errors`); `__hot_cold_t` overloads are not recognized;
  default arguments of a user-declared allocation function are not
  used (every parameter must be matched by an argument).

### 5.10 Immediate functions (`consteval.rs`, WP-133)
clang's `CheckForImmediateInvocation`, `HandleImmediateInvocations` and
P2564 escalation ([expr.const]/16-20, [dcl.constexpr]/2).
- **Declarations.** `consteval` is accepted on functions, member
  functions, constructors, conversion functions, lambdas (`[]()
  consteval {}`) and function templates, and dumped as clang does
  (`consteval`; an escalated function is `constexpr immediate`, a
  lambda's escalated `__invoke` `implicit immediate`). Errors: on a
  variable or field (`consteval can only be used in function
  declarations`), a destructor, `main`, a parameter (`function parameter
  cannot be constexpr|consteval|constinit`), with `constexpr`, and a
  redeclaration that disagrees. A consteval virtual function is allowed
  ([class.virtual]/19). `FnSpec::is_immediate()` (consteval or
  escalated) is what codegen skips.
- **Immediate function context** ([expr.const]/16): a consteval body,
  the constant-evaluated branch of `if consteval`, an unevaluated
  operand, and the manifestly constant-evaluated contexts the parser
  brackets with `enter/exit_constant_evaluated` (`static_assert`,
  array bounds, template arguments, the `if constexpr` condition,
  `parse_constant_expr`). Nothing is wrapped or checked there.
- **Invocations.** Building a call, member call, operator call or
  construction of an immediate function outside such a context wraps
  it in an unvalued `ConstantExpr` and records it as a candidate
  (`check_immediate_invocation`); the callee's `DeclRefExpr` is dropped
  from the references. `finish_immediate_invocations` runs at the end of
  every full-expression (`act_on_full_expr`): nested candidates lose
  their wrapper, the outermost are evaluated with `eval_const`
  (`EvalKind::Consteval`) and the value stored in the `ConstantExpr`.
  The initializer of a variable that is constant-initialized keeps its
  wrappers unvalued (clang's deferred mode). A failure reports `call to
  consteval|immediate function 'f' is not a constant expression` with
  the Oven's notes. Default arguments and default member initializers
  are not checked at their definition (`note_default_init_full_expr`).
- **Default-argument temporaries** ([class.temporary]/6, WP-216): a
  default argument is its own full-expression at the definition
  (`ExprWithCleanups`), but each use takes over its cleanups: the
  `CXXDefaultArgExpr` child is the argument without that wrapper
  (`Ast::default_arg_expr`, clang `ParmVarDecl::getDefaultArg`) and
  `note_default_use` marks the using full-expression as needing cleanups
  (clang `CheckCXXDefaultArgExpr`), so `g()` with `void g(const X& =
  X())` destroys the `X` after the call. Codegen, the Oven and the AST
  dump read the argument through `default_arg_expr`.
- **Default uses** ([expr.const]/16, [dcl.fct.default]/5, [class.mem]/10;
  WP-137). Each use of a default argument (`convert_call_args`,
  `ctor_args`) or default member initializer (aggregate `field_filler`,
  a constructor's implicit member init in `collect_field_init`) whose
  definition holds an unvalued invocation is recorded with
  `note_default_use` and checked at the end of the using
  full-expression (`check_default_use`): each invocation is evaluated
  afresh, and a failure is `call to consteval function 'h' is not a
  constant expression` at the invocation, then `in the default
  initializer of 'p'` at the use (callee begin, construct loc, ctor
  name, or the closing `}`/`)` of an aggregate init) and `declared here`
  at the parameter or field. A default used inside another default
  argument or initializer is checked at that definition only. In an
  immediate-escalating function (defaulted or implicit constructor,
  `constexpr` template specialization, lambda) the failure escalates
  instead; a constructor's reason note is `'D' is an immediate
  constructor because the default initializer of 'm' contains a call to
  a consteval function 'h' and that call is not a constant expression`.
  An instantiation re-checks the defaults its shared expressions and
  rebuilt init lists use (`templ/subst/shared.rs`, `subst/exprs.rs`),
  so a template use is reported at the definition and at each
  instantiation. Codegen folds each use separately (WP-144). clang 19's
  extra report at a class template's NSDMI instantiation and inside
  `sizeof` are not replicated.
- **References.** A `DeclRefExpr` (or `&C::m`) naming an immediate
  function that is not the callee of a candidate is `cannot take
  address of consteval function 'f' outside of an immediate
  invocation`, with `declared here`. For a qualified name (`&C::m`, also
  inside a member function) the error is at the qualifier, as clang.
  The Oven also rejects a constant result (or a subobject of one) that
  is a pointer, reference or member pointer to an immediate function:
  `pointer|reference to a consteval declaration is not a constant
  expression` at the invocation's callee, then `declared here`
  ([expr.const]/22; WP-137).
- **Escalation** (P2564, [expr.const]/18-20). Immediate-escalating:
  a lambda call operator, a defaulted special member, a `constexpr`
  function instantiated from a templated entity, and a `constexpr`
  function template while its definition is parsed (not a member of a
  class template's pattern, clang's `TK_NonTemplate`: its failed
  invocation is an error at the definition; WP-227). In one, a failed invocation
  or a bad reference makes the function immediate (`FnSpec::escalated`)
  instead of an error: the failed `ConstantExpr` loses its wrapper and
  its callee `DeclRefExpr` (or the `CXXConstructExpr`) is flagged
  `immediate-escalating` (dumped), and the reason is kept for the note
  `'f' is an immediate function because its body contains a call to ...`
  on a later failed call of `f`. Successful invocations in it are still
  wrapped and folded. A specialization starts non-escalated whatever
  its pattern is; refs already flagged are skipped. A lambda whose call
  operator escalates gets an escalated `__invoke`, and its conversion
  function to a function pointer is escalated too
  ([expr.prim.lambda.closure]/10; clang 19 instead rejects every use of
  the conversion of a consteval lambda).
- **Templates.** A template definition's non-dependent invocations are
  checked at the definition (they are shared with every instantiation),
  including those of a constructor's mem-initializers, which are not
  built until instantiation (sema-class.md §4.9); dependent ones are
  built and checked at instantiation, with `in instantiation of ...`
  notes. A shared invocation that failed at the definition is evaluated
  again by each instantiation that substitutes it
  (`recheck_shared_invocation`): reported again with the instantiation's
  context, as clang reports its rebuilt `ConstantExpr`, or escalating an
  immediate-escalating instantiation (`e<int>` of `template<class T>
  constexpr int e() { return bad(1); }` becomes immediate, so a
  non-constant call of it is an error; WP-227). A `ConstantExpr` is substituted
  through to its operand and re-wrapped by the rebuilt call. A shared
  (not rebuilt) expression is walked at each instantiation and the
  functions it odr-uses (named, constructors, bound destructors,
  `operator new`/`delete`) are marked referenced, which defines or
  instantiates them, as clang's rebuild does (`templ/subst/shared.rs`;
  unevaluated operands and lambda bodies are not entered).
- **Codegen.** A valued `ConstantExpr` lowers to its constant: scalars
  directly, aggregates by clang 19's `CreateCoercedStore` into the
  destination (one store per top-level element).
- Gaps: the escalation of a generic lambda's conversion function is not
  tracked per specialization; an implicit constructor does not escalate
  through a member whose (non-NSDMI) default construction is a failed
  consteval constructor call.

### 5.11 Atomics (`builtins/atomic.rs`, WP-154)
- **Builtins.** `__c11_atomic_*` and `__atomic_*` calls (the ids in
  `kiln_ast::decl::builtin::atomic`) become an `AtomicExpr`, as clang's
  `Sema::BuildAtomicExpr`: the pointer operand is checked
  (`sema_atomic_builtin_must_be_pointer`, `_op_needs_atomic` for the c11
  forms, `_needs_non_const_atomic`/`_pointer`, incomplete types, `_BitInt`
  prohibited), the value kind per op (int / int-or-fp / int-ptr-or-fp /
  trivially copyable), value operands are converted to the value type (or
  `ptrdiff_t` for pointer arithmetic, or pointers for the generic
  `__atomic_load`/`store`/`exchange`/`compare_exchange` forms), orders to
  `int`, and a constant order invalid for the op is
  `-Watomic-memory-ordering` (`sema_atomic_op_invalid_memory_order`). The
  result type is clang's (void, the value, or `bool`).
- **Lvalues of `_Atomic(T)`.** Lvalue-to-rvalue conversion adds
  `AtomicToNonAtomic`; assignment and initialization from `T` add
  `NonAtomicToAtomic`. Operand checks of the arithmetic, shift, bitwise,
  unary and `switch` conditions use the value type (`value_ty`). `++`/`--`
  and compound assignment are allowed on a modifiable `_Atomic` lvalue
  and yield an lvalue of `T` (`++`) or `_Atomic(T)` (compound assignment),
  as clang 19's `decltype` shows. A braced list initializing an `_Atomic`
  object is `sema_illegal_initializer_type`, as clang, unless it holds
  one element of class type.
- **`_Atomic(T)` of class type `T` (WP-165).** Initialized from a `T` or
  a class derived from `T` (copy, direct, or a one-element braced list)
  as `T` is, then `NonAtomicToAtomic` (clang `InitializeFrom`
  `NeedAtomicConversion`; `Sema::atomic_class_init`); a `return` operand
  too (`convert_for_init`, WP-183), while an argument is not converted
  (clang's implicit conversion sequence has no such step, so the call is
  "no matching function"); other class sources fail as a conversion to
  `_Atomic(T)`. Assignment from `T` or `_Atomic(T)`
  is the right side's lvalue conversion then `NonAtomicToAtomic`
  (`conv::atomic_class_assign`, C11 6.5.16.1). Assignment errors to any
  `_Atomic` target are reported at the operator, as clang's C path.
- `-Watomic-alignment` is reported by codegen (spec/codegen.md §7d).

### 5.12 `__builtin_va_arg` (`builtins.rs`, WP-171)
- **`act_on_va_arg(list, ty, ty_loc, loc, rparen)`** is clang's
  `Sema::BuildVAArgExpr`. The operand initializes a
  `__builtin_va_list &` (an AAPCS64 struct) or, for the x86-64 array
  `va_list`, decays and must be `__va_list_tag *` (otherwise the
  reference-binding / conversion error). The type must be complete
  (`sema_va_arg_incomplete`, with the forward-declaration note),
  non-abstract (`sema_va_arg_abstract`, with the pure-virtual notes) and
  POD (`sema_va_arg_non_pod`, `-Wnon-pod-varargs`, an error by default;
  a reference type is non-POD). A type that undergoes default argument
  promotion (`va_arg_promotion`: `float`, or a type whose integral
  promotion differs from it, an enum compared through its underlying
  type, so `E : short` warns and `enum class` does not) is
  `sema_va_arg_promotable` (`-Wvarargs`) through `diag_runtime_behavior`.
  Dependent operands build a dependent `VAArgExpr`; substitution rebuilds
  through `act_on_va_arg`.
- **Runtime-behavior diagnostics** (`stmt.rs`, clang
  `DiagRuntimeBehavior`): inside a function body they are queued on
  `FnState::runtime_diags` and flushed in `end_function_body`; outside a
  function (a namespace-scope initializer) they are emitted at once.
  `runtime_diag_quiet` drops them where clang's innermost evaluation
  context is not potentially evaluated (WP-235):
  - an unevaluated operand;
  - a manifestly constant-evaluated expression: a constant-expression
    the parser enters with `enter_constant_evaluated` or sema with
    `act_on_constant_full_expr`, and the conversions of a non-type
    template argument. The Oven reports the constant-evaluation failure
    instead (`division by zero` as a note of the error);
  - a quiet context of the body, counted by `FnState::rt_quiet`: the
    body of a `consteval` function, the immediate compound statement of
    `if consteval` / `if !consteval`, and the branch a non-dependent
    `if constexpr` condition discards (`enter_discarded_statement`,
    from the parser's `constexpr_condition_value`).

  A variable's initializer, including its conversions, is potentially
  evaluated again (`enter_var_initializer` zeroes `rt_quiet` until
  `act_on_init` is done; clang's `ActOnCXXEnterDeclInitializer`). So
  `if constexpr (false) { int x = 1 / 0; }` still warns, as in clang.
  Each queued diagnostic has an optional anchor expression; an anchored
  one is dropped at the flush when the anchor is unreachable (see the
  unused-value reachability rule, WP-203).
  **Gaps:** the `diag_runtime_behavior` callers queue without an anchor,
  so clang's CFG reachability filter does not apply to them
  (`if (0) va_arg(ap, char)` is silent there, reported here), and a
  non-dependent expression shared by a template and its instantiation is
  checked once, at definition, except for `-Wnull-dereference` (§5.15).
  At the end of a body in a dependent context the queue is flushed
  unfiltered for a function template declared by name. For any other
  function (a member of a class template, a lambda or local class in a
  template, a generic lambda's call operator) the anchored entries are
  dropped, and their instantiations warn instead: clang finishes a
  `FunctionTemplateDecl` without an analysis policy, and its
  `IssueWarnings` returns early for a dependent context. The unanchored
  ones (`-Wvarargs` `va_arg` promotion, `-Wuninitialized` self-reference)
  are still reported at the definition, because the instantiation shares
  and does not re-check them (clang reports them per instantiation).

### 5.13 Vector conditionals (`vector_ops.rs`, WP-173)
The vector operators themselves are summarized in spec/types.md
(`Vector`). `?:` follows clang 19:
- **Vector condition** (`act_on_vector_conditional`, clang
  `CheckVectorConditionalTypes`): a condition of vector type with an
  integral element is decayed, not converted to `bool`, and selects lane
  by lane.
  - A `void` or `throw` arm is `GNU vector conditional operand cannot be
    void|a throw expression`.
  - Two vector arms must have the same type (`vector operands to the
    vector conditional must be the same type`, with clang 19's stray
    `}`), and must be ext vectors exactly when the condition is
    (`cannot mix vectors and extended vectors in a vector conditional`).
  - One vector arm: the scalar arm is converted as for a binary vector
    operator (`CheckVectorOperands`, `bool` vectors allowed).
  - Two scalar arms: the same type, or the usual arithmetic conversions.
    An enumeration result is `enumeration type T is not allowed in a
    vector conditional`. The result is a vector with the condition's lane
    count and kind (ext or GNU), and both arms are splatted. A
    non-arithmetic result is `incompatible operand types`, a deviation:
    clang 19 builds a malformed vector or crashes.
  - Then the result must have the condition's lane count (`vector
    condition type T and result type U do not have the same number of
    elements`) and lane size (`... do not have elements of the same
    size`).
  - Codegen: a GNU condition is `icmp ne 0` plus `select`. An ext
    condition is the sign-bit mask blend (`icmp slt 0`, `sext`, `and`,
    `or`, with float arms bitcast to the condition type), as clang
    emits it for C++. A `bool` ext-vector condition is *M4*.
- **Scalar condition with a vector arm** (`scalar_cond_vector_operands`):
  converted to `bool` as usual, and the arms are converted as for a
  binary vector operator without `bool` vectors. A `bool`-vector arm is
  `invalid operands to binary expression`.

### 5.14 `-Waddress-of-packed-member` (`packed.rs`, WP-183)

clang's `RefersToMemberWithReducedAlignment`, `DiscardMisalignedMemberAddress`
and `DiagnoseMisalignedMembers`:

- **Candidates.** `&m` (parentheses stripped) of a `MemberExpr` chain
  `a.b.c` is recorded when all of these hold:
  - every link names a non-static data member (not an indirect field);
  - some link is packed, either the member or its class;
  - the top base, with parentheses and implicit casts stripped, is a
    `DeclRefExpr` or `this`;
  - the member's type is aligned to more than 1 byte;
  - the summed field offsets are not a multiple of that alignment, or
    the outermost class's alignment is lower.

  For `.` on a non-reference variable, the outermost class's alignment
  is raised to the variable's own alignment (type or `alignas`). The
  culprit is the innermost member that is packed or is in a packed
  class. Its alignment is the smaller of its type's and its class's.
- **Discard.** A conversion of the recorded `&m` drops it when the
  target is an integer, a dependent type, or a pointer to an incomplete
  type or to a type aligned no more than the culprit:
  - every cast node in the full-expression (implicit and explicit);
  - `implicit_convert` in any context except simple assignment (clang's
    `InitializationSequence::Perform` step, even for an identity
    conversion);
  - both operands of a pointer-pointer comparison.

  A conditional operator's arms, a pointer arithmetic operand, `(void)`
  and the right side of `=` do not discard.
- **Report.** At the end of the full-expression (`act_on_full_expr`), at
  the `MemberExpr`'s begin location: `taking address of packed member
  'm' of class or structure 'C' may result in an unaligned pointer
  value`. An anonymous class is named by its typedef.
- **Deviations:**
  - A non-dependent `&m` in a template is reported once, at the
    definition. clang repeats it in every instantiation.
  - An anonymous union or struct member as the culprit prints as `''`.
    clang prints `C::(anonymous union at ...)`.
- Test: `tests/conformance/expr.unary.op/address-of-packed-member.cpp`.

### 5.15 `-Wnull-dereference` (`null_deref.rs`, WP-191)
The two syntactic `*null` checks of clang 19
(`CheckForNullPointerDereference`). The pattern is a `*p` found under the
expression with parentheses, casts, full-expressions and materialized
temporaries ignored, where `p` (also with those ignored) is a null
pointer constant: a zero integer literal, a `std::nullptr_t` value or
`__null` (`(1 - 1)` is not one).
- **Indirection** (`check_null_indirection`): run from
  `conv::lvalue_to_rvalue` (clang `DefaultLvalueConversion`) and on the
  left operand of a builtin simple or compound assignment (clang
  `CheckAssignmentOperands`; not for `++`/`--`). The operand must have
  pointer type and the dereference must not be volatile. The warning
  "indirection of non-volatile null pointer will be deleted, not trap"
  [-Wnull-dereference] carries the operand's range and the note
  "consider using __builtin_trap() or qualifying pointer with
  'volatile'", both at the `*`.
- **Reference binding** (`check_null_reference_binding`): run where
  `conv::bind_reference` binds a glvalue directly (clang
  `SK_BindReference`): "binding dereferenced null pointer to reference
  has undefined behavior" [-Wnull-dereference], volatile or not. A
  binding through a converted temporary (`const long &r = *(int *)0`)
  loads instead and gets the indirection warning.
- Both go through `diag_if_reachable` and are dropped in an unevaluated
  operand or an immediate function context (§5.1 reachability). Member
  access, subscripts, `&*`, `(void)` and discarded operands load nothing
  and are not diagnosed.
- **Templates** (WP-235): a dependent `*(T *)0` is checked when the
  instantiation rebuilds it. A non-dependent one is shared by the
  instantiation (spec/sema-templates.md §5.4), so the definition keeps
  each warning it computed, keyed by the dereference and numbered in
  check order (`null_deref_rewarn`). The walk that marks a shared
  expression's functions referenced (`mark_shared_expr_referenced`,
  which skips unevaluated operands and lambda bodies) collects the
  recorded dereferences it meets and queues their warnings again, in
  that order, under the instantiation's notes and reachability, unless
  the context is quiet. It costs nothing while no template has warned.
  Inside an instantiation these warnings are exempt from the engine's
  deduplication against the definition's report (spec/diag.md §7.4)
  and deduplicated per dereference and instantiation context instead
  (`null_deref_seen`), so a shared dereference that a rebuilt parent
  re-checks warns once. Other run-time warnings (division by zero,
  shift counts) are not repeated, as in clang.
- **Deviations:** A default member
  initializer of a class template does not warn again when a
  constructor uses it (clang instantiates it then), and a variable
  template specialization's warning can print later than clang's,
  which instantiates it at its first use.
- Tests: `tests/conformance/expr.unary.op/null-dereference-indirection.cpp`,
  `tests/conformance/dcl.init.ref/null-dereference-binding.cpp`,
  `tests/conformance/temp.inst/null-dereference-rewarn.cpp`.
### 5.16 GNU `_Complex` (`complex.rs`, `conv/`, WP-211)
clang 19's C99/GNU complex support in C++:
- **Types.** `_Complex` combines with `float`, `double`, `long double`
  and the integer types (`char` through `long long`, signed or unsigned).
  Plain `_Complex` is `_Complex double` with a warning. Any other base
  (`bool`, `void`, `wchar_t`, `char8_t`, `char16_t`, `char32_t`,
  `__int128`, `auto`, `__bf16`, a typedef name, spelled `'type-name'`) is
  `sema_invalid_complex_spec`, and the declaration continues with the base
  type (so `_Complex void v;` also reports the incomplete type).
  `-pedantic` warns `'_Complex' is a C99 extension` on each keyword
  (parser, `-Wc99-extensions`).
- **Traits** (clang): `__is_arithmetic`, `__is_fundamental`,
  `__is_scalar` and `__is_aggregate` are true for every complex type;
  `__is_floating_point` is true for a complex floating type;
  `__is_integral`, `__is_signed` and `__is_unsigned` are false.
- **Standard conversions** (`conv/scs.rs`): complex promotion
  (`_Complex float` to `_Complex double`, Promotion rank), complex
  conversion, and complex-real conversion in either direction (a rank
  worse than Conversion), plus complex-to-`bool`. The cast kinds are
  clang's `*RealToComplex`, `*ComplexCast`, `*ComplexToReal`,
  `*ComplexToBoolean`, `IntegralComplexToFloatingComplex` and back.
  Sugar on the target type is kept in the cast type.
- **Usual arithmetic conversions** (`conv/arith.rs`, clang
  `handleComplexConversion`): a real floating operand keeps its realness
  and only its precision is promoted. An integer operand of a complex
  floating type, and a real operand of a complex integer type, become
  complex. Two complex integer types take the elements' common integer
  type without promotion (`_Complex unsigned char + _Complex unsigned
  char` stays `_Complex unsigned char`). A compound assignment's left
  operand is not converted.
- **Operators.** Complex operands allow unary `+ - ~` (`~` is the
  conjugate), `!`, binary `+ - * /`, `==`/`!=`, `&&`/`||`/`?:` conditions,
  assignment and compound assignment. `<`/`>` and `%` are invalid
  operands. `++`/`--` are allowed; under `-pedantic` they give
  `'++' on an object of complex type is a C2y extension`
  (`-Wc2y-extensions`) at the operator.
- **`__real e` / `__imag e`** (clang `CheckRealImagOperand`): the element
  type of a complex operand, or a real arithmetic operand's own type.
  `__real` keeps the operand's value category. `__imag` does for a
  complex operand, and is a zero prvalue otherwise.
- **`__builtin_complex(re, im)`**: two operands of the same real floating
  type `T` give `_Complex T`. Other operand types are diagnosed as clang
  does.
- **List initialization** (clang `CheckComplexType`): `{re, im}`
  initializes the parts, each converted as a list element with narrowing
  checked. A third clause is excess. One clause initializes from a
  scalar.
- **GNU imaginary constants.** The lexer sets `imaginary` for an
  `i`/`j` suffix (spec/lex.md). `act_on_{integer,float}_literal` builds
  the plain literal wrapped in an `ImaginaryLiteral` of `_Complex T`, and
  emits `sema_imaginary_constant` ("imaginary constants are a GNU
  extension", `-Wgnu-imaginary-constant`, off by default) at the literal.
  When the suffix is also a valid ud-suffix (`1i`, `1.0if`, `1il`), literal
  operator lookup runs first. If no cooked, raw or template form exists,
  sema falls back to the imaginary constant without a diagnostic (clang
  `LookupLiteralOperator` with `DiagnoseMissing = false`). The `<complex>`
  operators therefore still win where they are visible.
- **`-Wuser-defined-literals` tail.** "no literal will invoke this
  operator" is omitted for any valid ud-suffix: the numeric library
  suffixes and `sv`, using clang's `StringLiteralParser::isValidUDSuffix`.
  Sema has no `-std`, so the C++20 set is used.
- Tests: `tests/conformance/ext.complex/` (`types`, `constexpr`,
  `diagnostics`, `pedantic`, `imaginary`).

### 5.17 Value-dependent static member calls and dependent-base members (WP-211)
- A call to a static member function of the current instantiation is
  value-dependent ([temp.dep.constexpr]/2.3). An `if constexpr` on it in a
  member initializer is deferred to instantiation, as libc++
  `mdspan`'s `__rank_` needs
  (`tests/conformance/temp.dep.constexpr/static-member-function-current-instantiation.cpp`).
- An overloaded `B::f` named through a dependent base that is not `B`
  is called without an object argument. It is diagnosed with clang's
  `diagnoseInstanceReference` wording ("call to non-static member
  function without an object argument").

## 6. Initialization (`init.rs`)
- **`perform_init(ty, init, direct_list)`** implements [dcl.init]:
  - copy- and direct-initialization of scalars, through the implicit
    conversion sequence, with narrowing enforced for list-init
    ([dcl.init.list]/7)
  - references, binding directly or to a temporary
  - character arrays from string literals: the bound is deduced, and
    an over-long string is an error
    (`initializer-string for char array is too long`, two arguments)
  - aggregates, with brace elision, designators (C++20, in order) and
    unions (first member, or the designated one); an aggregate class
    whose list holds one undesignated element of the same or a derived
    class type is initialized from that element ([dcl.init.list]/3.2)
  - vector members (WP-173, clang `CheckSubElementType`): a GNU vector
    member cannot be initialized from a scalar, so a scalar starts a
    brace-elided list of up to N elements (`elided_vector_init`). A
    designator after its first item ends that list. A vector initializer
    copy-initializes the member, and an ext vector member splats a scalar
    instead. A vector item inside a vector's element list, braced or
    elided, is `cannot initialize a vector element of type T with an
    rvalue|lvalue of type V`. Deviation: clang 19 does not stop the list at
    a designator, so `{1, 2, .z = 5}` is an error there (a `void`
    "vector element"). The C99 mixture of designated and positional
    items is diagnosed on the written list in both cases (below).
  - a non-class `T` list-initialized from one class-type element (or
    `bool` from `nullptr`) is initialized from the element itself
    (`class_element_list_init`; clang's core-24034 rewrite in
    `TryListInitialization`): direct-initialization for a direct list
    (an explicit conversion function is a candidate), failures at the
    entity (variable name, `T` of `T{e}`, the `{` of an argument or
    return value; an argument adds its "passing argument" note), no
    `-Wbraced-scalar-init`, and narrowing checked on the user
    conversion's result
- **Designators** (WP-192; clang `SemaInit.cpp`). The parser reads
  `.name`, nested `.a.b`, `[i]` (C99), the GNU range `[a ... b]`
  (WP-193; `-Wgnu-designator`, an Extension shown under `-pedantic`, at
  the `...`) and mixtures of them. A leading
  `[` begins a designator unless it begins a lambda (clang
  `MayBeDesignationStart`): `[=`, `[...` and `[]` are lambdas, and
  `[&`, `[*` or `[identifier` scan to the matching `]` and are a
  designator when `=` follows.
  - Syntactic warnings come from `act_on_init_list_syntax`, once per
    written list at parse time, never on instantiation. All are
    `-Wc99-designator` (in `-Wc99-extensions`):
    - nested designators: at the first designator's `.`
    - array designators: at the `[`
    - the mixture of designated and positional items: at the designator
      begin, with a note on the first non-designated item, or, when
      item 0 is designated, at item 0 with the note on the positional
      item. A designated pack expansion `.m = e...` counts as
      non-designated here, as in clang.
  - Designation checks (WP-193; clang `ActOnDesignatedInitializer`) run
    in `act_on_designation` when the parser has read a designation,
    on each non-dependent index (`designator_index`), all at the
    index's begin:
    - not integral or unscoped enumeration type: `integral constant
      expression must have integral or unscoped enumeration type, not T`
    - not constant: `expression is not an integral constant expression`
      with the evaluator's notes, unless it folds (`fold_not_ice`)
    - negative: `array designator value '-1' is negative`
    - an empty range: `array designator range [3, 1] is empty`, at the
      `...`.
    A failed designation makes the parser drop the whole braced list,
    after reading it through its `}` (clang `ParseBraceInitializer`).
    Nothing more is diagnosed for that list (no syntactic warnings, no
    conversion of its clauses), and its declaration is invalid. The
    same checks run again on instantiated indices in `check_array`.
  - Semantic checks run in `check_aggregate_in` and `check_array`,
    again at every instantiation. A designator names a direct field or
    a field of an anonymous member (`designated_field`). A nested or
    anonymous designator descends into the subobject (`Sub { braced:
    false, entry, prior }`), and the positional items that follow keep
    filling it (`.p.y = 1, 7, 8` sets `p.z` and `w`). A subobject named
    again merges with its prior initialization, seeded from the prior
    `InitListExpr` (`prior_list`). Seeded slots count as explicit unless
    they are fillers (`ImplicitValueInitExpr`, `CXXDefaultInitExpr`).
    Descending into a subobject that an expression initialized keeps
    that object and re-initializes only part of it ([dcl.init.aggr]/4,
    C99 6.7.9p19): the slot becomes a `DesignatedInitUpdateExpr` whose
    base is the old initializer and whose updater is an `InitListExpr`
    of the subobject's type, with `NoInitExpr` in the slots it does not
    touch (an array updater runs to its highest index, with
    `NoInitExpr` holes and filler; a union updater holds only the
    designated member). Lists nested in an updater fill with
    `NoInitExpr` too (`Sub::update`, clang `FillWithNoInit`), except a
    braced value, which value-initializes, and a merged prior list,
    which keeps its own fill (`is_update_list`). The next designator
    warns `initializer partially overrides prior initialization of
    this subobject` with the old initializer noted; keeping the old
    object of a class with a non-trivial destructor is an error
    (`initializer would partially override prior initialization of
    object of type T with non-trivial destruction`). A full override
    of an initializer that may have side effects notes that they will
    not occur. Codegen emits the base and then the updater; the Oven
    does not evaluate it, as clang's evaluator does not (`constexpr
    variable must be initialized by a constant expression`).
    - out of declaration order: `-Wreorder-init-list` (ExtWarn, in
      `-Wreorder`), at the later value, with the note
      `previous initialization for field` on the earlier one. It covers
      only the list's own designators.
    - a re-initialized subobject: `-Winitializer-overrides` at the new
      value, with the note `previous initialization is here`. A union
      initialized through a different member is the DefaultError form,
      at the designator.
    - Order and override diagnostics are substitution failures
      whatever their severity (clang's `SFINAEFailure` ExtWarns;
      `is_sfinae_failure`), so a constraint or a SFINAE'd signature
      using them fails. The `-Wc99-designator` warnings are not.
    - In overload resolution ([over.ics.list]/8), a list that
      initializes a subobject twice converts to no aggregate
      (`designators_override`, clang `InOverloadResolution`): the same
      path, a whole object after a part of it, a part after the whole,
      another member of the same union, through anonymous members.
      Order does not matter there, and a braced value's own designators
      are not compared.
    - an unknown field: `field designator 'q' does not refer to any field
      in type T`, at the name.
    - a field designator into a scalar: `field designator cannot
      initialize a non-class type`, at the `.`.
    - an array designator into a class: `array designator cannot
      initialize non-array type`, at the `[`.
    - an index (or range end) at or past a known bound: `array
      designator index (5) exceeds array bounds (3)`, at that index.
    - a range `[a ... b]` initializes each element from the same
      clause, re-checked per element; positional items continue at
      `b + 1`.
    - an array designator leaving a hole gives the list an
      `array_filler`, even when the element count equals the bound.
    - non-aggregate types are `designated_non_aggregate`.
    - a union initialized by `{}` initializes the variant member with a
      default member initializer, directly or within an anonymous
      member (`union_default_member`, [dcl.init.aggr]/5; clang
      `CheckStructUnionTypes`), and otherwise value-initializes its
      first member.
  - In templates (spec/ast.md §5.6, spec/sema-templates.md §5.4), the
    designators are carried in `DesignatedInitExpr`s and in the written
    form of non-dependent lists. The semantic checks run on the
    instantiated type at the written locations. A designated pack
    expansion gives one designated clause per element. A non-dependent
    list that the instantiation shares rather than rebuilds keeps the
    order and override diagnostics its definition check issued
    (`designator_rewarn`), and each instantiation issues them again
    under its instantiation note (`reissue_designator_warnings`), as
    clang's rebuilt list does; a rebuilt list warns again by itself.
    Neither is deduplicated against the definition's.
  - Deviations:
    - A range designator whose clause has side effects evaluates the
      clause once per element. clang's codegen refuses it ("cannot
      compile this GNU array range designator extension yet"), and GCC
      evaluates it once.
- **`init_from_list`** builds clang's semantic `InitListExpr`, with
  `array_filler`.
- **References** (`reference_list_init`, [dcl.init.list]/3.10): when
  the single element is not reference-related, a temporary of the
  referenced type is list-initialized with the reference's own kind
  (direct for `T& r{..}`, copy for `T& r = {..}`, a call argument or
  a `return`). A failed constructor call for that temporary names the
  reference type (`no matching constructor for initialization of
  'const E &'`, and `call to deleted constructor of 'const J &'`), via
  `ctor_diag_ty`, which `use_ctor` takes on entry so nested
  constructions cannot see it. While an aggregate (class or array)
  temporary is list-initialized, the first `excess elements` error in
  it takes clang's `in initialization of temporary of type 'const H'
  created to list-initialize this reference` note at the reference
  (`ref_temp_note`, clang's `diagnoseListInit`); a scalar temporary
  (`const int& r = {1, 2};`) gets no note. Too many elements for a
  scalar report at the initialization's location: the declarator, the
  `{` of a scalar member's nested list (`H h = {1, {2, 3}};`), and the
  `{` of a default member initializer
  (`tests/conformance/dcl.init.list/{reference-temporary-errors,scalar-excess-locations}.cpp`).
- **Results** are attached with `act_on_init`. `act_on_uninit` handles
  default-initialization: const objects without an initializer are an
  error, and incomplete types are diagnosed.
- **Constexpr variables:** `check_constexpr_init` evaluates the
  initializer of a `constexpr` variable, and of a const integral
  variable, which is usable in constant expressions. The value is
  stored (§8.3).
- **Self-reference** (`self_ref.rs`, WP-171; clang `CheckSelfReference`):
  after the initialization is performed, `act_on_init` walks the
  converted initializer of a variable without automatic storage
  duration, or of class or reference type, for uses of the variable
  itself ([basic.life]/7). A use warns
  `variable {0} is uninitialized when used within its own
  initialization` (namespace scope, or class type),
  `reference {0} is not yet bound to a value ...` (references, any use)
  or `static variable {0} is suspiciously used ...` (static locals,
  `-Wstatic-self-init`, inside `-Wuninitialized`), through
  `diag_runtime_behavior`. As in clang: a value use is an
  lvalue-to-rvalue conversion, increment/decrement, compound-assignment
  operand, overloaded-operator argument, copy-constructor argument,
  `std::move` argument, or a non-static member function call on the
  variable; `T a = a;` with non-class `T` and copy-initialization is
  the silencing idiom; a member read in an aggregate initializer list
  is fine once that member is initialized; unevaluated operands,
  `constexpr` variables, static data members of non-class type and
  variables in a linkage specification (clang checks the declaration
  context) are not diagnosed. Other locals are left to a CFG
  uninitialized-values analysis kiln does not have.

### 6.1 Braced lists in expressions and explicit type conversions (`forms.rs`)
- **Call arguments** `f({...})` ([expr.call]/7):
  - `act_on_braced_arg` returns a `void` `InitListExpr` placeholder,
    kept in `braced_args`.
  - `act_on_call` replaces it by the list-initialization of the
    parameter.
  - Viability follows [over.ics.list]: a scalar takes at most one
    unbraced, convertible element; an aggregate at most as many
    elements as brace elision reaches. A non-viable list is clang's
    `cannot convert initializer list argument to 'T'` candidate note,
    except that a single class-type element for a non-class parameter
    notes the element's own conversion ([over.ics.list]/10: `no known
    conversion from 'N' to 'int' for 1st argument`).
  - A list for the ellipsis of a variadic function is an error.
- **`return {...};`** ([stmt.return]/2, `act_on_return_list`):
  copy-list-initialization of the returned object.
- **`x = {...}`** ([expr.assign]/8, `act_on_assign_list`): the list
  initializes a temporary of the left operand's type, which is then
  assigned. A type-dependent left operand or list element makes it a
  dependent `BinaryOperator` with the written list as its right operand,
  assigned again from the substituted list at instantiation.
- **Other operators:** a braced list as their operand is ill-formed
  (`act_on_misplaced_init_list`).
- **Functional casts** ([expr.type.conv]):
  - `T()` (`act_on_value_init_cast`) value-initializes; `void()` is
    a `void` prvalue.
  - `T{...}` (`act_on_list_cast`) is direct-list-initialization.
    For an enumeration with a fixed underlying type that includes
    [dcl.init.list]/3.8 (`E{v}`: `v` converted to the underlying type,
    narrowing rejected, then to `E`), as `E e{v};` does.
  - `T(a, b)` (`act_on_paren_list_cast`) is ill-formed for a scalar.
    The expression-list may hold braced-init-lists (`T(a, {b})`, `T({})`).
    They are the placeholders of `act_on_braced_arg`, and the
    construction list-initializes its parameter from them. A dependent
    operand keeps the list as written (`dependent_init_list`), and
    substitution makes it a placeholder again. A lone parenthesized list
    for a non-class `T` is `sema_list_init_in_parens` (clang's
    FK_ParenthesizedListInitForScalar).
  - Class, array and reference `T` need temporaries and are
    unsupported (`temporary_unsupported`) until classes have
    constructors (WP-065).
  - Array `T{...}` (`array_list_cast`, WP-216; `using A = int[2]; A{1,
    2}`) list-initializes an array prvalue that keeps the cv-qualifiers
    of `T` ([expr.type]/2). An array of class type with a non-trivial
    destructor is bound (`CXXBindTemporaryExpr` of the array type,
    `maybe_bind_temporary` descends to the element type), and so is a
    braced list bound to a reference to array (`reference_list_init`).

## 7. Statements (`stmt.rs`)
- **Compound statements and scopes:** see §2.
- **`if`, `while`, `for`:** condition conversion to `bool` (contextual
  conversion), plus `cond_var` for condition declarations
  (`act_on_condition_var`).
- **`act_on_if(kind, init, cond_var, cond, then, els, loc)`** builds
  every `IfKind`:
  - **`if constexpr`** ([stmt.if]/2): `act_on_constexpr_condition(e)`
    converts the condition contextually to `bool` and evaluates it with
    `EvalKind::IfConstexpr`; the result is `ConstantExpr(cond, value)`.
    A failure is `constexpr if condition is not a constant expression`
    at the condition's first token, with the Oven's notes. In M2 both
    branches are analyzed and kept; the discarded statement
    ([stmt.if]/2: not instantiated in a template, return statements not
    used for deduction) is M3, with templates. Codegen and the Oven
    emit only the selected branch.
  - **`if consteval` / `if !consteval`** ([stmt.if]/4): no condition;
    both substatements are compound statements (the parser enforces it:
    `expected { after consteval` / `expected { after else`). The
    compound executed when constant-evaluated (the `then` branch of
    `if consteval`, the `else` branch of `if !consteval`) is an
    immediate function context ([expr.const]/16):
    `enter_immediate_context`/`exit_immediate_context` bracket it and
    `in_immediate_context()` reports it, and its immediate invocations
    are neither wrapped nor checked (§5.10). In a template the branches
    are substituted the same way at instantiation.
- **`switch`:**
  - integral promotion of the condition
  - duplicate `case` values (with a note)
  - multiple `default` labels
  - `case` values converted to the promoted type
  - `act_on_start_switch(loc, …)` and
    `act_on_finish_switch(…, cond_var)` bracket the body
  - GNU case ranges `case lo ... hi` (WP-205): the parser emits the
    `-Wgnu-case-range` extension at the `...`; `act_on_case_range`
    converts both bounds like a case value and `act_on_case(value,
    Some(hi), …)` builds the `CaseStmt`. At the end of the switch
    (`check_case_ranges`, clang `ActOnFinishSwitchStmt`) the ranges are
    sorted by `lo`; an empty one (`lo > hi`) is `empty case range
    specified` and dropped; one overlapping a case value reports the
    largest such value, and one overlapping the previous range reports
    that range's `hi`, both as `duplicate case value` at the range with
    a `previous case defined here` note. Template instantiation goes
    through the same entry points.
  - `-Wswitch-bool` (`check_switch_bool`, before promotion): a
    condition clang's `isKnownToHaveBooleanValue` accepts (a `bool`,
    `!`, a comparison, `&&`/`||`, a bitwise or `?:` of such, an
    assignment/comma whose result is such) warns `switch condition has
    boolean value` at the `switch` keyword.
  - Enumeration and constant checks (`switch_enum.rs`
    `check_switch_cases`, WP-219), in clang `ActOnFinishSwitchStmt`
    order, after `check_case_ranges`; skipped for a dependent or invalid
    condition and for a value-dependent case label (clang
    `HasDependentValue`: -Wswitch-default included, so a template whose
    labels depend on a parameter warns only per instantiation, WP-230):
    1. no `default`: `-Wswitch-default` (off by default) `'switch'
       missing 'default' label`;
    2. nothing more if the case list is broken (an unfolded label, a
       duplicate value, an overlapping range);
    3. no `default` and a constant condition: `no case matching constant
       switch condition 'N'` (no group) when no case or range holds it;
       nothing more either way;
    4. only a complete enumeration with enumerators (the condition
       before promotion) continues;
    5. a case value, then each range's `lo` and `hi`, that is no
       enumerator's value: `case value not in enumerated type 'E'`
       [-Wswitch]; a label naming a const variable of static or
       thread storage duration of the same enumeration is exempt, and so
       is every value of an `enum_extensibility(open)` enumeration; for a
       `flag_enum` one "an enumerator's value" means any value whose bits
       are all bits of its positive single-bit enumerators (clang
       `ShouldDiagnoseSwitchCaseNotInEnum`, `IsValueInFlagEnum`);
    6. enumerators sorted by value (first of equal values kept),
       deprecated, `[[maybe_unused]]` and unused ones skipped, not
       covered by a case or range: none and a `default` is `default
       label in switch which covers all enumeration values`
       [-Wcovered-switch-default, off] for a closed non-flag enumeration
       (clang `isClosedNonFlag`); otherwise `enumeration value 'A'
       not handled in switch` (two: `values 'A' and 'B'`, three: `'A',
       'B', and 'C'`, more: `N enumeration values not handled in switch:
       'A', 'B', 'C'...`) [-Wswitch] without a `default`, or the same with
       `not explicitly handled` [-Wswitch-enum, off] with one, at the
       condition's expression location.
    Like clang these repeat in every instantiation (emitted with
    `allow_duplicate`).
- **Jumps:**
  - `break` and `continue` outside a loop or switch are errors.
  - `return`:
    - a value in a `void` function is an error
    - a missing value in a non-`void` function is an error
    - the value is converted as if by copy-initialization
- **Falling off the end:** see §4.3.

### 7.4 Labels and goto
- Labels are `LabelDecl`s in the function, with payload
  `Payload::None(0)`.
- `goto` to an undeclared label is reported at the end of the function
  (`use of undeclared label 'x'`).
- A redefinition is an error, with a note.
- **Landed (WP-153): jump-scope checking** (`jump_scope.rs`, clang
  `JumpScopeChecker`), run at the end of every function body with a
  label, `switch` or `asm goto` (in templates, at instantiation).
  Protected scopes: a variable whose initialization a jump would bypass
  ([stmt.dcl]/3: non-trivial default init, any initializer, a VLA or a
  non-trivial destructor), try blocks and handlers ([except.handle]/2),
  each arm of a constexpr/consteval `if` ([stmt.if]/2), GNU statement
  expressions and lifetime-extended temporaries with a destructor. A
  `goto` or `case` entering one is `cannot jump from this goto statement
  to its label` / `cannot jump from switch statement to this case label`
  with one `jump bypasses …` note per scope entered; an `asm goto` may
  neither enter nor leave a scope with a destructor (`cannot jump from
  this asm goto statement to one of its possible targets`, the
  `possible target of asm goto statement` note and `jump exits …`
  notes). Leaving scopes through `goto` is fine; codegen runs their
  cleanups (spec/codegen.md).
- `[[noreturn]]`: the fall-off analysis (§4.3) treats a call as not
  returning when any redeclaration of the callee is `[[noreturn]]`,
  including member calls.

### 7.5 Landed (WP-100): try-blocks (`try_stmt.rs`)
- `act_on_exception_decl` ([except.handle]/1-3): an rvalue-reference
  type is `cannot catch exceptions by rvalue reference`; (a pointer or
  reference to) an incomplete type other than cv `void` is
  `cannot catch [pointer|reference] to incomplete type T`; an abstract
  class is the usual abstract-variable error; array and function types
  decay. A class type by value is copy-initialized from an
  `OpaqueValueExpr` (clang's `ExceptionDecl` init) and its destructor is
  checked (access, deleted).
- With `-fno-exceptions`, `try` is `cannot use 'try' with exceptions
  disabled` (the statement is still analyzed).
- `act_on_try`: a `catch (...)` that is not last is `catch-all handler
  must come last`; a handler whose type is the same as, or a public
  unambiguous derived class of, an earlier one gets `-Wexceptions`
  `exception of type T will be caught by earlier handler` with the note
  `for type U` (clang `CheckCXXTryBlock`, references and pointers
  compared by their pointee). Exact duplicates use clang's
  `CatchHandlerType` key (WP-160): the top-level cv-qualifiers are dropped
  first, then a reference or pointer is stripped and the pointee keeps its
  cv-qualifiers. So `int` then `const int &` is not a duplicate, while
  `const int` then `int &` is. The public-base search ignores cv.
- `push_try_scope` marks the scope a try barrier (`Scope::try_barrier`)
  for the implicit move of thrown locals (§5.5).
- Function-try-blocks: `check_function_try_block` (from
  `act_on_finish_function_def`) diagnoses `return` in a constructor's
  handler (`return in the catch of a function try block of a
  constructor is illegal`) and warns (`-Wexceptions`, WP-118) at each
  member access whose base is `this` (written or implicit, under
  implicit casts only, so not `(this)->m` or `(*this).m`, and not in a
  lambda) in a constructor's or destructor's handler: `cannot refer to
  a non-static member from the handler of a constructor|destructor
  function try block` ([except.handle]/10; clang
  `IsInFnTryBlockHandler`, once for a template at its definition);
  falling off a constructor or destructor
  handler rethrows ([except.handle]/14), so the fall-off analysis
  (§4.3) treats such handlers as not returning, and a value-returning
  function's handler that falls off is `-Wreturn-type`.
- Analysis warnings ([except.spec]/5, [dcl.attr.noreturn]/2), skipped
  in templates: a `throw` that can escape a non-throwing function is
  `-Wexceptions` `'f' has a non-throwing exception specification but
  can still throw` with `function declared non-throwing here` (or the
  destructor/deallocator note); in an instantiated function template
  specialization the name carries its arguments, unqualified (`'k<int>'`,
  `except.spec/throw-in-nothrow-specialization.cpp`); a `[[noreturn]]`
  function that can fall off is `-Winvalid-noreturn`. Only throws reachable in clang's CFG
  count (`try_stmt/reach.rs`, WP-118): a structured walk ends the live
  path at `return`/`break`/`continue`/`goto`/a throw/a `noreturn` call;
  prunes edges by `tryEvaluateBool` (constant folding, `&&`/`||` decided
  by one side, `checkIncorrectLogicOperator`) in `if`, loops, `?:`,
  `&&`/`||`; reaches only the matching case of a constant switch and not
  the end of a switch with no default over a fully covered enumeration
  (`isAllEnumCasesCovered`); reaches a handler only from a live throw in
  its try-block (or in a nested try without a catch-all, as calls have
  no exception edges); gives a live `asm goto` an edge to each of its
  labels besides the next statement (WP-230); iterates `goto` labels and loop back edges to a
  fixed point; and checks nothing in a function with an `if consteval`
  (clang builds no CFG). The operands of `sizeof`/`alignof` are not
  walked (a `noexcept` operand is, as clang's CFG does); a GNU case range
  is reached when the constant condition is in `[lo, hi]` and counts
  toward enumeration coverage (WP-205). Not modeled: implicit
  destructors marked `noreturn`; indirect `goto` needs no model because
  kiln does not parse `&&label` / `goto *p`. The fall-off analysis
  behind -Wreturn-type / -Winvalid-noreturn (`falls_off` →
  `fall_off_kind`, WP-219) is the same walk run to a fixed point,
  classifying the live predecessors of the exit as in clang
  CheckFallThrough: a live `return`/`co_return`; a throw with no
  enclosing try (fake edge); a live `noreturn` call (abnormal edge); a
  live try dispatch with no catch-all and no enclosing try (abnormal
  edge; a dispatch with no predecessor, i.e. no throw and no nested try
  without catch-all in its block, counts as live, as clang's CFG
  rescans it); and the end of the body (plain edge). No plain edge is
  `Never`; a plain edge plus any other is `Maybe` (`... in all control
  paths`); a plain edge alone is `Always`. A function with `if
  consteval` is `Never`.
- Initialization (one condition in `init.rs`, flagged): brace elision
  ([dcl.init.aggr]/16) only descends into an aggregate element; a class
  with a user-provided constructor is copy-initialized from the element
  instead (`brace-elision-class.cpp` AST golden).
- **Left (WP-100):** a written vs implicit `noexcept` on deallocators in
  the note. (The handler member-reference warning, throw reachability,
  `arr()` member value-initialization of a class array and the
  `new K[2][3]` constructor initializer landed in WP-118.)

### 7.6 Landed (WP-089): range-based `for` (`range_for.rs`)
Sema desugars `for (init; decl : range) body` ([stmt.ranged]/1) the way
clang's `ActOnCXXForRangeStmt`/`BuildCXXForRangeStmt` does, into a
`CXXForRangeStmt` (ast.md, `RangeForData`) holding `__range{d}`
(`auto&&`), `__begin{d}` and `__end{d}` (`auto`), the condition
`__begin != __end`, the increment `++__begin` and the loop variable
initialized from `*__begin`. `d` is clang's scope depth of the for's
control scope halved (`clang_scope_depth()/2`, with the depth outside the
template saved in `range_for_outer_depth` for instantiations).

- **Parser contract.** `parse_for` pushes the control scope and scans the
  head for a top-level `:` (`at_for_range_head`). The range path calls
  `act_on_declarator` for the loop variable, then
  `act_on_for_range_decl(ds, v)` (storage classes:
  `loop variable 'x' may not be declared 'static'`; a type definition:
  `types may not be defined in a for range declaration`), parses the
  range inside `begin_for_range_init(extend)`/`end_for_range_init`
  (P2718 collection, sema-class.md §5.4), and calls
  `act_on_for_range(init, loop_var, range, temps, colon, loc)`, which
  returns a `RangeForHead`; after the body,
  `act_on_finish_for_range(head, body)` builds the statement. A failed
  head (`None`) or a failed declaration (`act_on_for_range_error`) marks
  the loop variable invalid and the body is still parsed. `for (x : r)`
  is `range-based for loop requires type for loop variable` (the
  variable gets type `auto`); a structured binding is declared by
  `act_on_decomposition_declarator` (§4.9); an
  init-statement before C++20 is `-Wc++20-extensions`.
- **Range kinds.** An array range (a constant array; a VLA is
  unsupported) uses `__range` and `__range + N` (`N` a `ptrdiff_t`
  literal). A class range uses member `begin`/`end` when the class has
  either member ([stmt.ranged]/1.3, P0962), else ADL-only calls
  `begin(__range)`/`end(__range)` (`build_begin_end`), each checked for
  viability before it is built. A braced-init-list range deduces
  `std::initializer_list<E>` for `__range`.
- **Diagnostics (clang's text and locations).**
  `invalid range expression of type T; no viable 'begin'|'end' function
  available` with the overload candidates as notes; a lone member is
  `member is not a candidate because range type T has no 'end'|'begin'
  member`; an incomplete class is `cannot use incomplete type T as a
  range` with the forward-declaration note; a pointer whose pointee is a
  valid range is `invalid range expression of type T; did you mean to
  dereference it with '*'?` and the loop is rebuilt on `*range`
  (`rebuild_with_dereference`); an iterator or loop-variable failure gets
  `selected 'begin' function with iterator type T` (the candidate notes
  are captured through `diag_capture` while each attempt runs and
  released on success of the diagnosis). A SFINAE context entered
  inside a capture (deeper on the instantiation stack than where the
  capture began, such as deducing a converting constructor template
  for a rewritten `operator==` candidate of `__begin != __end`) takes
  its errors as substitution failures, not captured diagnostics
  (WP-271).
- **Templates.** A type-dependent range (or loop-variable type) keeps
  only `__range` and the loop variable in the pattern; instantiation
  (`templ/subst/stmts.rs` `range_for`) substitutes the range under a new
  P2718 collection, declares the loop variable, and finishes the head
  with `instantiate_for_range`. A non-dependent range is desugared in
  the definition.
- **Fixed on the way (WP-089).** Reference binding and reference ICS
  compare reference-relatedness on types made unqualified through array
  levels (clang `getUnqualifiedArrayType`), so `int[3]` binds to
  `const int (&)[3]`. `auto` deduction under a reference or pointer
  declarator keeps an array result as `Auto` sugar deduced to the
  cv-unqualified-through-arrays array (clang's `AutoType`), so
  `auto &r = a;` prints `int &[2]` as clang does; the dump printer keeps a
  dependent `auto` (deduced as `<dependent type>`) as `auto`.
- **Left (WP-089):** `err_range_on_array_parameter` (needs
  `ParamData.original_ty`, which `act_on_param_declarator` does not set,
  so an array parameter gets the generic `'int *'` diagnostic);
  VLA ranges; the default-on
  `-Wempty-body`-style `warn_empty_range_based_for_body` and the
  DefaultIgnore `-Wrange-loop-construct`/`-Wrange-loop-bind-reference`
  warnings (groups declared, not emitted); the begin/end-types-differ
  pre-C++17 extension warning; clang's recovery for `for (int x, y : a)`.

### 7.7 Landed (WP-140): GNU asm statements (`asm.rs`)
`act_on_gcc_asm_stmt` follows clang's `ActOnGCCAsmStmt`: output and input
constraints are validated per target (`kiln_abi::asm`, aarch64 and
x86_64), outputs must be modifiable lvalues
(`invalid lvalue in asm output`), a register-only input is converted to a
prvalue (lvalue-to-rvalue) and a memory input stays an lvalue, an
immediate input (`i`, `n`) must be an integer constant expression, a
tied operand must have a compatible type, and the template is parsed
(`%N`, `%[name]`, `%=`, `%%`, modifiers) with clang's errors for an
invalid operand number, unknown name and a mismatched modifier (the
aarch64 `-Wasm-operand-widths` warning with its `%w` fix-it note).
Clobbers are checked against the target's register names. Each operand
ends a full-expression (its odr-uses are resolved). The `!srcloc`
values clang attaches are precomputed into `AsmData.srclocs` (literal
begin, then every later template line; offset + 2). Templates are
substituted (`subst/stmts.rs`) and rechecked at instantiation.
`act_on_file_scope_asm` builds a `FileScopeAsmDecl`. Not implemented:
immediate range checks
(`validateInputSize`/`validateOutputSize`), `=@cc` flag outputs, register
variables (`register int r asm("x0")`) as operands and their clobber
conflict check, the C++17 "asm in constexpr function" extension warning;
`!srcloc` values in headers and macro expansions do not match clang's
encoding.

**Deferred (WP-160 item 4): macro `!srcloc` parity.** For a string from a
macro expansion, clang emits `2^31 + offset`, where the offset is into
clang's unified source-location space. That space holds, in order: entry 0
(2 bytes), the main file (size + 1), the predefines buffer (20928 bytes on
aarch64 for C++26, which is the `-dM` text plus 4213 bytes of other
builtins), and then one entry of length + 1 per macro expansion. Matching
it needs two things:
- kiln_source keeping a clang-compatible offset counter across files.
- The same sequence of expansion entries as clang: argument
  pre-expansion runs, the 4060-byte scratch buffer, expansions inside
  `#if`, and included files.

That is an ABI-free, preprocessor-wide change with no effect on codegen.
The `!srcloc` value only labels inline-asm backend diagnostics, so it is
left for a pp/source WP. WP-182 re-evaluated this: kiln_source and
kiln_pp still have no clang-compatible offset space (expansions are
recorded per `ExpansionInfo`, not as a running offset), so the deferral
stands.

### 7.8 Landed (WP-140): predefined identifiers (`predefined.rs`)
`act_on_predefined_expr` is clang's `BuildPredefinedExpr`: the enclosing
function (or, outside one, the translation unit with
`-Wpredefined-identifier-outside-function`); a dependent context gives a
dependent `PredefinedExpr` that template instantiation rebuilds;
otherwise a `const char[N]` lvalue holding the name.
`compute_predefined_name(kind, f)` is clang 19's
`PredefinedExpr::ComputeName`: `__func__`/`__FUNCTION__` are the bare
name (`operator int`, `operator()`, `top level` is only pretty);
`__PRETTY_FUNCTION__` prints `virtual `/`static `, the return type
(`auto` for lambdas, none for constructors and destructors, the
instantiation pattern's otherwise), the scope with specialization
arguments (`A<int>::`), the pattern's parameters (`, ...` if variadic),
cv and ref qualifiers, and ` [T = int, ...]`: enclosing implicit class
specializations' arguments (outermost first) then the function template
specialization's, printed with their types (`2U`, `'a'`, packs
`<int, char *>`, `x:auto` for a generic lambda's invented parameter).
The source-location builtins use the same printer (§9.5 of
sema-templates.md), and a `__builtin_FUNCTION`/`__builtin_source_location`
in a dependent context, or a default argument evaluating one used there,
is marked instantiation-dependent and recomputed at instantiation.
Tests: `tests/conformance/dcl.fct.def.general/predefined-*.cpp`,
`expr.const/builtins-srcloc-template-run.cpp`, `tests/codegen/predefined.cpp`.
Gap: a function with a deduced return type prints `auto` where clang
evaluates the source-location name after deduction.

### 7.9 `-Wempty-body` (`empty_body.rs`, WP-191)
clang 19's `DiagnoseEmptyStmtBody` / `DiagnoseEmptyLoopBody`. The body is
a null statement whose `;` is spelled on the presumed line of the
statement's `)` (for `switch`, of the condition's end). The warning
("if statement has empty body", "switch statement has empty body",
"range-based for loop has empty body", "for loop has empty body",
"while loop has empty body" [-Wempty-body]) and the note "put the
semicolon on a separate line to silence this warning" are both at the
`;`.
- The parser calls `check_empty_body` for an `if` without `else` (not
  `if consteval`; `if constexpr` counts) and a range-based `for`, and
  `check_empty_switch_body` for a `switch`, when the statement is built.
- A `for` or `while` is recorded (`note_loop_body`, in
  `Sema::empty_loop_bodies`). When its enclosing compound statement is
  finished, `check_empty_loop_bodies` warns if the next statement of
  that block is a compound statement or begins in a later presumed
  column than the loop. `do ; while (c);` never warns.
- A `;` preceded by a macro that expanded to nothing (`if (c) EMPTY;`)
  does not warn. clang reads `NullStmt::hasLeadingEmptyMacro`, a
  preprocessor token flag. kiln reads the source instead: anything other
  than whitespace and comments between the `)` and the `;`. A `;` that
  comes from a macro is spelled on another line and does not warn.
- Nothing is diagnosed in a template instantiation; a template definition
  warns once.
- **Deviation:** a block-scope declaration with leading attributes
  (`[[maybe_unused]] int q;`) begins at its decl-specifiers in kiln, so
  a loop before it may warn where clang (which begins at the `[[`) does
  not.
- Test: `tests/conformance/stmt.pre/empty-body-warning.cpp`.

### 7.10 `-Wunreachable-code` (`unreachable_code.rs`, WP-234)
clang 19's `CheckUnreachable` (`ReachableCode.cpp`) with Sema's
`UnreachableCodeHandler`, run with the other flow warnings at the end of
a function body, never in a dependent context, in an instantiation or
after an error (`uncompilable_error_count`, spec/diag.md §7.5).
- `Sema::dead_scan` (in `try_stmt/reach.rs`) follows control through the
  body as clang's CFG does. A constant branch condition prunes its dead
  edge unless it is a configuration value (`is_configuration_value`:
  a `constexpr` call, a global or `const` local, `sizeof`, or a
  parenthesized or macro-expanded literal).
- Each round reports one dead root: the last dead element, in
  evaluation order, that no reached or dead element flows into. It is
  then treated as reached and the scan repeats, so the regions come out
  last first, as clang scans its blocks in reverse. The cost is
  O(regions × body).
- A root that is a `break` is `'break' will never be executed`
  [-Wunreachable-code-break]; a `return`, or part of a returned value
  whose evaluation does not branch, is `'return' will never be executed`
  [-Wunreachable-code-return]; otherwise `code will never be executed`
  [-Wunreachable-code]. The warning is at clang's `GetUnreachableLoc`:
  an operator's token, a member's name, else where the element begins.
- Not reported: a root in a macro expansion, the literal or enumerator
  condition of `do { ... } while (0)`, or a reference to
  `__builtin_unreachable`.
- A dead `for` increment is `loop will run at most once (loop increment
  never executed)` [-Wunreachable-code-loop-increment] at the increment.
- A dead branch pruned by a bare literal condition has the note
  `silence by adding parentheses to mark code as explicitly dead`, with
  the `/* DISABLES CODE */ (` ... `)` fix-its around the literal. A root
  whose note repeats the previous root's literal is dropped.
- A dead `[[fallthrough]];` is reported by -Wunreachable-code-fallthrough
  (§ implicit fallthrough) instead when that is enabled.
- Groups: -Wunreachable-code = {-Wunreachable-code-fallthrough,
  -Wunreachable-code-loop-increment}; -Wunreachable-code-aggressive adds
  -break and -return. None is in -Wall.
- Tests: `tests/conformance/stmt.return/unreachable-code*.cpp`,
  `stmt.break/unreachable-code-break.cpp`,
  `stmt.if/unreachable-code-config.cpp`.

### 7.11 `-Wfor-loop-analysis` (`for_loop_analysis.rs`, WP-234)
clang 19's `CheckForLoopConditionalStatement` and
`CheckForRedundantIteration`. `act_on_for` runs both as each `for` is
built, so a template definition is checked, and every instantiation is
checked again (the warnings are `allow_duplicate`, so each
instantiation repeats them, as clang does).
- **Condition** (not with a condition variable). The condition must be
  simple (clang `DeclExtractor`): literals (integer, floating, `bool`,
  character, `__null`, imaginary), casts, parentheses, built-in
  unary operators except `*`, binary and compound-assignment operators,
  `?:` and `?:` with an omitted middle operand, and names of variables
  or parameters. Unevaluated operands (`sizeof`, `noexcept`, an
  unevaluated `typeid`) are skipped. Anything else (a member, a call, an
  array subscript, an enumerator, `nullptr`, a template parameter) is
  not simple and nothing is checked.
- None of those variables may be `volatile` or lack automatic storage
  (a parameter has it; a `static`, `thread_local` or namespace-scope
  variable does not).
- The condition, the increment and the body are then searched (clang
  `DeclMatcher`, an evaluated-expression walk): any `return`, `break`
  or `goto`, or any use of one of the variables that is not read
  through an lvalue-to-rvalue conversion (an assignment, `++`, `&x`,
  binding a reference, a discarded-value `x;`), silences the warning.
  A read through `?:` arms counts as a read. Lambda bodies are not
  searched, only their capture initializers.
- Otherwise: `variable 'i' used in loop condition not modified in loop
  body`, `variables 'i' and 'n' ...`, up to four names, and `variables
  used ...` with five or more [-Wfor-loop-analysis] at the first name.
- **Redundant iteration.** The body is a braced, non-empty block whose
  last statement and the increment are both `++x`/`x++` (or both
  `--`), built-in or overloaded, applied directly to the same name (no
  parentheses). Unless the body has a `continue` of this loop (a nested
  `switch` is searched; nested loops other than their init-statements
  are not): `variable 'i' is incremented both in the loop header and in
  the loop body` [-Wfor-loop-analysis] at the body's name, with the note
  `incremented here` at the header's name.
- Groups: -Wloop-analysis = {-Wfor-loop-analysis,
  -Wrange-loop-analysis}; not in -Wall.
- Tests: `tests/conformance/stmt.for/for-loop-analysis.cpp`,
  `stmt.for/loop-analysis.cpp`.

### 7.12 `-Wbitfield-constant-conversion` (`bitfield_constant.rs`, WP-234)
clang 19's `AnalyzeBitFieldAssignment`. An integral or enumeration
constant (implicit casts and parentheses stripped, folded) stored into a
bit-field of width N that cannot hold it reports `implicit truncation
from T to bit-field changes value from V to W` [-Wbitfield-constant-
conversion, on by default; a subgroup of -Wconstant-conversion], where T
is the constant's type and W the value of its low N bits read with the
bit-field's signedness ([conv.integral]/3). Storing 1 into a one-bit
wide signed bit-field reports `... to a one-bit wide bit-field ...`
[-Wsingle-bit-bitfield-constant-conversion, a subgroup].
- Nothing is reported for a `bool` bit-field, a dependent or invalid
  bit-field, a value-dependent or non-constant value, or a value of
  128 or more bits.
- The value needs the width of its type, except that a negated or
  complemented constant (`-5`, `~0u`), or any unsigned constant written
  that way, needs only its significant bits (`APInt::getSignificantBits`
  of the bit pattern), so `u2 = -1` and `u4 = ~0u` are silent.
- **Sites.** A simple assignment `b = v` (not a compound one), at the
  operator, from the implicit-conversion walk of the full-expression
  (`impcast.rs`); when it is reported the conversions of `v` are not
  analyzed further. The initialization of a bit-field member (clang
  `CheckBitFieldInitialization`): aggregate initialization (including
  designated and brace-elided elements, `new T{...}`) and parenthesized
  aggregate initialization, at the element; a mem-initializer, at the
  member name; a default member initializer, at the initializer (the
  `{` of a braced one). A braced scalar initializer `{v}` is reported at
  `v` with `v`'s type and again at the initialization's location with
  the bit-field's declared type (clang checks the element and then the
  `InitListExpr`; the significant-bits rule does not apply to the
  second).
- Each template instantiation is checked again (`allow_duplicate`).
- **Gaps.** A default member initializer of a class template
  specialization is instantiated with the class rather than at its
  first use, so its warnings carry `in instantiation of template class`
  instead of clang's default-member-initializer notes. The enumeration
  checks of `AnalyzeBitFieldAssignment` for a non-constant value
  (`-Wbitfield-enum-conversion`) and `-Wsigned-enum-bitfield` are not
  implemented.

## 8. Constant evaluation (`oven_bridge.rs`)

Sema does no constant evaluation of its own: every constant it needs
comes from the Oven (`kiln_oven`, spec/oven.md; "Integration" there
covers the Oven's side). `constfold.rs` keeps only integer helpers
(`int_info`, `wrap`, `fits`). The M2 `Folder`, its tree-walking `Eval`
interpreter and `aggval` were removed by WP-054.

### 8.1 One Oven per translation unit
- `Sema` owns one `Oven`, in `Sema.oven: Option<Oven>`. Compiled
  bytecode, resolved globals and the memo cache are shared by every
  evaluation in the TU.
- `oven_bridge.rs` implements `SemaBridge` for the duration of one call.
  `with_oven` takes the Oven out of its `Option` and hands the call a
  `Bridge` that holds `&mut Sema`:
  - `ensure_body(f, oven)` returns the body of the function's
    definition. Since WP-070b, a never-marked specialization or member
    of a class specialization is instantiated first, during a
    manifestly constant-evaluated evaluation only. Sema lends the
    Oven back by swapping it into `Sema.oven` while it instantiates, so
    nested evaluations run on the same Oven (R-OVEN-1;
    spec/sema-templates.md §4.11);
  - `ensure_layout` uses `kiln_abi::ensure_layout`;
  - `diag` emits into the engine (for `-Wenum-constexpr-conversion`).
- `set_constexpr_limits(Budget, backtrace_limit)` applies the driver's
  `-fconstexpr-*` flags, reaching Sema through
  `kiln_parse::ConstexprLimits`.
- `oven()` and `into_parts() -> (Ast, Oven)` hand the Oven's statistics
  to the driver for `-fconstexpr-report`.

### 8.2 Entry points
| method | Oven call | use |
|---|---|---|
| `eval_const(e, EvalContext)` | `evaluate` | manifestly constant-evaluated contexts; recorded in the stats and the report; failures carry notes |
| `eval_int(e, ctx)` | `evaluate` | the same, for an integral or `bool` result |
| `try_fold(e)` / `fold_int(e)` | `try_fold` | silent, unrecorded speculative folds for warnings |
| `is_constant_initialized(var)` | `is_constant_initialized` | [basic.start.static]/2, usable-in-constant-expressions checks |
| `const_int(v)` / `const_truth(v)` | (reads `ast.consts`) | the integer or truth value of a scalar constant |

### 8.3 Evaluation sites
Each site picks the `EvalContext` that clang 19 uses there. The context
sets the report `kind` and whether an out-of-range conversion to an
unscoped enumeration is reported (spec/oven.md "Integration").

| construct | context |
|---|---|
| `static_assert` condition | `EvalKind::StaticAssert` |
| initializer of a `constexpr` variable | `EvalContext::init(var, loc)`; on success the variable becomes `VarEvalState::Value(id)` |
| `case` label, enumerator of an enumeration with a fixed underlying type, `noexcept(e)` specifier | `EvalContext::converted(loc)`; a converted constant expression ([expr.const]/12) |
| `if constexpr` condition | `EvalKind::IfConstexpr` |
| array bound, bit-field width, enumerator without a fixed type, `alignas`, array designator | `EvalKind::Other` |

- **GNU folding** (`Sema::fold_not_ice`, clang
  `VerifyIntegerConstantExpression` with `AllowFold`, WP-230): for a
  bit-field width, an enumerator without a fixed underlying type and an
  array designator, an expression that is not an integral constant
  expression but folds laxly (`fold_rvalue_int`: a `const` non-volatile
  variable reads as its initializer, e.g. `(int)d` for `const double d =
  2.0`) is accepted with its folded value. It warns `expression is not an
  integral constant expression; folding it to a constant is a GNU
  extension` [-Wgnu-folding-constant] (an Extension, so off unless
  `-pedantic` or the group is enabled), with the evaluator's notes, at
  the expression's begin (a lone "subexpression not valid" note moves
  the location instead). Array bounds have their own folding (`variable
  length array folded to constant array`, above). A `case` label,
  `static_assert` and `alignas` never fold.

- **Speculative folds (`try_fold`):**
  - narrowing checks in list-initialization and fixed-type enumerators
  - division by zero and shift-amount warnings
  - the `static_assert` failed-comparison explanation (`'5 == 6'`)
  - constant `if`/loop conditions in the falls-off-the-end analysis
    (`-Wreturn-type`)
  - the alignment of an `alignas` expression for layout
- **Usable in constant expressions** ([expr.const]/4) means:
  - a `constexpr` variable, or
  - a `const`, non-`volatile` integral or enumeration variable,
  - whose initializer is a constant initializer
    (`is_constant_initialized`).

  A reference is usable when it binds to a static lvalue.
- **Failures:**
  - The primary diagnostic is sema's, at the site, as clang reports it
    (for example `static assertion expression is not an integral
    constant expression`).
  - The `EvalFailure`'s notes follow it (`with_failure_notes`): the
    reason, the `in call to` backtrace, and any `declared here` notes.
  - An expression that already contains errors is not evaluated.

## 9. Target (`target.rs`)
- `pub use kiln_abi::{Arch, Target}`.
- `target_for_types` maps `TargetTypes` to a layout target.
- `target_of(ast)` recovers the target an AST was built for.
- Layout is `kiln_abi`'s Itanium implementation (spec/abi.md).

## 10. To be implemented (M3+)
- **Classes:** special members and construction (WP-067), virtual
  functions and virtual bases (WP-078); members, `this`, access control
  and non-virtual bases landed in WP-066b. Design:
  [sema-class.md](sema-class.md).
- **Overloading:** overload resolution beyond arity, user-defined
  conversions, operator overloading.
- **Templates:** templates, concepts, packs; `auto` and `decltype`
  beyond the M2 basics. Design: spec/sema-templates.md (the type-trait
  list, including the clang 22 traits of WP-263, is its §9.1).
- **Other language features:** lambdas, coroutines, reflection,
  contracts. (Exceptions landed with WP-100: §5.5, §7.5.)
- **WP-064 follow-ups:**
  - prototype scope (§4.3)
  - `dynamic_cast` and polymorphic `typeid`
  - class and array temporaries (§6.1) and compound literals (§5.8)
  - `-Wconstant-conversion` and `-Wliteral-conversion` alongside
    narrowing errors in list-initialization (outside braced lists they
    landed: spec/sema-conv.md §9)
  - statement expressions: `-Wgnu-statement-expression`, jumps into
    one, and class-glvalue results
- **Constant evaluation** (what the Oven supports in M2 is in
  spec/oven.md):
  - the discarded statement of `if constexpr` ([stmt.if]/2), with
    templates
  - template arguments (`EvalKind::TemplateArg`)
  - constant-initialization of `constinit` variables
  - constant-initialization checks of non-`constexpr` variables with
    static storage and of local `const` integral variables
    ([basic.start.static]/2): codegen folds them with
    `fold_constant_init` (spec/oven.md), but Sema does not evaluate
    them, so `-Wconstant-evaluated` is not reported in their
    initializers as clang reports it
  - Sema still rejects some M2 constructs before evaluation: a
    reference bound to a temporary, a cast to a reference type,
    list-initialization of a temporary, and initialization of a class
    object from anything but a braced list
- **Rules not checked yet:**
  - jumps past initialization ([stmt.dcl]/3)
  - [dcl.align]/6, that `alignas` must be on the definition
  - full [basic.def.odr] for inline functions and variables across
    redeclarations
- **Using-declarations** (§4.8):
  - using-declarators at class scope that name base-class members
    (needs base classes), and their `typename`, `operator` and pack
    (`...`) forms, which are diagnosed as unsupported
  - overload sets through using-declarations beyond what M2 overloading
    supports
  - conflicts between a shadow and an enumerator of the same scope
  - `previousDecl` links for redeclared shadows and aliases in the dump
  - an attributed `[[...]] using` declaration as a block-scope statement

## 11. Tests
- `crates/kiln_sema/tests/`: per-rule tests that drive `Sema`
  directly, for example `constfold.rs`.
- `tests/conformance/expr.const/`: constant evaluation through the
  driver (WP-054), including `oven:` budgets.
- `crates/kiln_parse/tests/`: rules that are easier to state in
  source, including `sema.rs`.
- `diag:` conformance tests under `tests/conformance/`, which also
  pass with `--compiler clang++-19`.
- AST goldens under `tests/conformance/ast/`.

## 12. Open questions
- **Additive `kiln_ast` APIs added by WP-048:**
  - `label_stmt`/`goto_stmt`
  - `set_non_odr_use`, `replace_enumerator_init`, `mark_bitfield`
  - `typedef_for_anon`/`set_typedef_for_anon`
  - `mark_part_of_explicit_cast`
  - `ExprArena::update_string_init_type`

  These should be reviewed for the AST spec.
- **Orphaned cast nodes:** `cast_to` merges a same-kind cast into the
  existing node and orphans the node it replaced. The arena keeps
  those orphaned nodes.
- **`parse_file` location:** `parse_file` lives in kiln_parse. It
  could instead live in a `kiln_driver` crate once kiln_pp feeds the
  parser.
- **Where to fold constants:** constant-expression warnings (division
  by zero, shifts) fold eagerly in Sema. With the Oven, they could be
  computed lazily instead.
