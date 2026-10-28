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
- **Qualified lookup** ([basic.lookup.qual]) searches one context's
  table, plus that namespace's using-directives.
- **Redeclaration lookup** (`lookup_current_scope`) searches only the
  current scope. If the current scope is the outermost block of a
  control statement (a Block directly above a Control scope), it also
  searches the Control scope, so `if (int x = 0) { int x; }` is
  diagnosed as [stmt.pre]/5 requires.
- **Block-scope `extern` declarations** are recorded in `local_externs`.
  A later namespace-scope declaration of the same name redeclares them,
  per [basic.link]/6 and clang's local extern decls.
- **`classify_name`** gives the parser a name's category: type,
  namespace, value, or undeclared.

## 4. Declarations (`decl.rs`, `declspec.rs`)

### 4.1 DeclSpec and Declarator
- The parser fills a `DeclSpec` and a `Declarator` (clang's classes of
  the same names).
- `type_from_declspec_at` checks the specifier combination
  ([dcl.type.general]/2, [dcl.spec]) with clang's wording, and gives
  the base type.
- `Declarator` carries:
  - the name and an optional qualifier
  - `chunks`, innermost first
  - attributes, bit-width, `init_style`, and context

### 4.2 Declarator to declaration
`act_on_declarator` turns a `Declarator` into a declaration:
1. It applies the chunks from the base type outwards ([dcl.meaning]),
   checking:
   - arrays of references, functions, `void` or abstract types
   - functions returning arrays or functions
   - references to references, pointers to references
   - array bounds: converted constant expressions, which must be
     positive
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

### 4.3 Functions
- `act_on_start_function` and `act_on_finish_function` bracket a body.
- The body is checked for:
  - redefinition
  - incomplete parameter and return types (with a forward-declaration
    note)
  - falling off the end of a non-`void` function (a structural
    analysis, `falls_off`). This warning is suppressed when the
    function had errors.
- `main` must return `int` ([basic.start.main]/2). Falling off the end
  of `main` returns 0, so it never warns.

### 4.4 Tags, enumerations, bit-fields and anonymous records
- **`act_on_tag(kind, name, TagUse, …)`** implements [dcl.type.elab]
  and [class.name]:
  - A reference finds or injects the tag.
  - A declaration redeclares the tag in the current scope.
  - A definition checks for redefinition and kind mismatch.
- **Enumerations** ([dcl.enum]):
  - enumerators are typed as clang types them
  - a fixed underlying type gets converted-constant-expression checks
    and narrowing diagnostics
  - an unfixed one gets its type from the enumerators' range (the best
    type), and the enumerators are then retyped to the enumeration
  - `replace_enumerator_init` records the converted initializer
- **Bit-fields** ([class.bit]):
  - the width must be an integral constant expression
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
  - `used`, `noinline`, `always_inline`, `weak`, `const`, `pure`, `cold`
- **Ignored with a warning:**
  - unknown attributes: `unknown attribute 'x' ignored`. Scoped names
    print without their scope, as clang 19 does.
  - GNU `visibility`, `section` and `format`: `'x' attribute ignored`
- **Argument encoding** (`Attr.a`, `Attr.b`; see spec/abi.md §3.1):

  | kind | `a` | `b` |
  |---|---|---|
  | ALIGNED | `ExprId` of the alignment expression | raw `QualType` of `alignas(type-id)` |
  | NODISCARD, DEPRECATED | `ExprId` of the string-literal message | — |

- **Semantics:**
  - `nodiscard` applies to functions, records and enumerations. A
    discarded call, or a discarded by-value result of such a type,
    warns.
  - `deprecated` warns on use.
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
- **`static_assert`** ([dcl.pre]/10) requires a converted constant
  expression of type `bool`.
  - On failure it reports
    `static assertion failed due to requirement '…'`, with the
    condition printed by `TypeNames::write_expr`, plus the message if
    there is one.
  - Evaluation notes follow (§8).

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
  - **literals:** via `kiln_sema::literal`; user-defined literals are
    diagnosed with clang's `no matching literal operator`
  - **operators:**
    - unary and binary operators, including compound assignment
    - `?:`, the comma operator
    - increment and decrement
  - **calls:** argument conversion, arity notes, variadic default
    promotions
  - **member access:** `.` and `->` on records and through
    `IndirectField`
  - **subscripts:** either operand order
  - **`sizeof`/`alignof`:** of types and expressions, using
    `kiln_abi` layout
  - **casts:** C-style, functional, `static_cast`,
    `reinterpret_cast`, `const_cast`, with
    `mark_part_of_explicit_cast`
- **Constant-folding warnings:** division by zero, negative shifts and
  over-wide shifts warn when the operands fold.

### 5.1 Unevaluated operands
`unevaluated > 0` inside `sizeof` and `alignof` operands. Names there
are not odr-used.

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
    unions (first member, or the designated one)
- **`init_from_list`** builds clang's semantic `InitListExpr`, with
  `array_filler`.
- **Results** are attached with `act_on_init`. `act_on_uninit` handles
  default-initialization: const objects without an initializer are an
  error, and incomplete types are diagnosed.
- **Constexpr variables:** `check_constexpr_init` evaluates the
  initializer of a `constexpr` variable, and of a const integral
  variable, which is usable in constant expressions. The value is
  stored (§8.3).

## 7. Statements (`stmt.rs`)
- **Compound statements and scopes:** see §2.
- **`if`, `while`, `for`:** condition conversion to `bool` (contextual
  conversion), plus `cond_var` for condition declarations
  (`act_on_condition_var`).
- **`switch`:**
  - integral promotion of the condition
  - duplicate `case` values (with a note)
  - multiple `default` labels
  - `case` values converted to the promoted type
  - `act_on_start_switch(loc, …)` and
    `act_on_finish_switch(…, cond_var)` bracket the body
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
- Jumps past initializations are not checked yet (M3, [stmt.dcl]/3).

## 8. Constant evaluation (`constfold.rs`, `aggval.rs`)

### 8.1 The ConstEvaluator seam
- Sema evaluates only through
  `trait ConstEvaluator { fn evaluate(&self, &Ast, ExprId) -> Result<ConstVal, NotConstant> }`.
- `evaluator(&Target)` returns the M2 `Folder`. Switching to the Oven
  (WP-051) is a change to that one function.
- `NotConstant` carries clang's `note_constexpr_*` note, located where
  clang puts it, plus an optional declaration note.
- **Callers:**
  - array bounds, bit-field widths, enumerator values
  - `case` values, `static_assert`
  - `constexpr` initializers
  - converted constant expressions ([expr.const]/12), with narrowing
    checks
- **Folded values:** integral, `bool`, enumeration, `float`/`double`
  and null pointer values.
- **Reads of variables:** only of variables usable in constant
  expressions, with clang's notes when they are not (`read of non-const
  variable`, `initializer of 'x' is not a constant expression`).

### 8.2 Interpreter
- Calls to `constexpr` functions run in a tree-walking interpreter
  (`Eval`), whose frames hold scalar parameters and locals.
- It executes:
  - compound, declaration, expression, `if`, `while`, `do` and `for`
    statements
  - `switch`, `break`, `continue`, `return`
  - assignment and increment of locals
- **Limits:** recursion depth 512, call depth 512, 2^20 steps. Hitting
  a limit makes the expression non-constant.
- A call to a non-`constexpr` function, or to one without a
  definition, gives clang's note.
- This is a stand-in. The Oven (spec/oven.md) replaces it in M3.

### 8.3 Aggregate values
- **Storing:** `aggval` folds the semantic `InitListExpr` of a
  `constexpr` array or class object, leaf by leaf through the
  evaluator. The value is stored as a canonical `ByteImage` in
  `ast.consts`, laid out by `kiln_abi`, with relocations for null
  pointers. The variable's evaluation state becomes
  `VarEvalState::Value(id)`.
- **The M2 subset:**
  - arrays and non-union classes without bases or bit-fields
  - leaves that are integral, enumeration, `bool`, `float`, `double`,
    null-pointer or `nullptr_t` values

  Anything else (unions, bit-fields, string initializers, `long
  double`) is left unvalued *without* a diagnostic, for the Oven.
- **Reading:** an lvalue-to-rvalue conversion of a place inside a
  valued aggregate is read from the image (`aggregate_place`,
  `read_leaf`). The place can be:
  - the variable itself
  - `.member` of a non-bit-field
  - `[i]`
  - parentheses
- **Out-of-bounds indices** give clang's notes:
  - `read of dereferenced one-past-the-end pointer is not allowed in a
    constant expression` when `i == N`
  - `cannot refer to element i of array of N elements in a constant
    expression` otherwise

## 9. Target (`target.rs`)
- `pub use kiln_abi::{Arch, Target}`.
- `target_for_types` maps `TargetTypes` to a layout target.
- `target_of(ast)` recovers the target an AST was built for.
- Layout is `kiln_abi`'s Itanium implementation (spec/abi.md).

## 10. To be implemented (M3+)
- **Classes:** member functions, special members, `this`, access
  control, bases and virtual functions.
- **Overloading:** overload resolution beyond arity, user-defined
  conversions, operator overloading.
- **Templates:** templates, concepts, packs; `auto` and `decltype`
  beyond the M2 basics.
- **Other language features:** lambdas, exceptions, coroutines,
  reflection, contracts.
- **Constant evaluation:**
  - the Oven replaces §8 (WP-051)
  - consteval and immediate invocations
  - non-scalar locals
  - pointers to objects
  - union and bit-field values
- **Rules not checked yet:**
  - jumps past initialization ([stmt.dcl]/3)
  - [dcl.align]/6, that `alignas` must be on the definition
  - full [basic.def.odr] for inline functions and variables across
    redeclarations

## 11. Tests
- `crates/kiln_sema/tests/`: per-rule tests that drive `Sema`
  directly, for example `constfold.rs`.
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
