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
| `prototype_params` | parameter names of each parameter-declaration-clause being parsed (§4.3) |
| `pragma_packs` | `#pragma pack` directives seen, with their location and argument text (recorded for the class layout, sema-class.md §6.1) |
| `immediate` | depth of immediate function contexts ([expr.const]/16) entered by `if consteval` |
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
- **Block-scope using-directives** ([namespace.udir]/2): a local
  scope's `using namespace N;` is collected while walking outwards, and
  `N`'s members are searched at the nearest enclosing namespace (or the
  translation unit) that encloses `N`, together with that scope's own
  using-directives.
- **Qualified lookup** ([basic.lookup.qual]) searches one context's
  table, plus that namespace's using-directives.
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
  current scope, and leaves out using-shadows
  (`current_scope_shadows` returns those, for §4.8's conflict checks). If the current scope is the outermost block of a
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
  - `chunks`, innermost first. `Chunk::Function` carries the
    parameters, `variadic`, the parenthesis locations and `exc`, the
    exception specification Sema built from the declarator (§4.7);
    `ExcSpec::None` when none was written (WP-058 added the field)
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
- **Exception specifications** ([except.spec]/5): function types that
  differ only in their exception specification are not "different
  types". When the new declaration writes none and the previous one is
  non-throwing the error is `'f' is missing exception specification
  'noexcept'` (the previous specification as written); any other
  difference is `exception specification in declaration does not
  match previous declaration`. Both take a `previous declaration is
  here` note, and the new declaration is marked invalid. Compatible
  spellings (`noexcept`, `noexcept(true)`, `throw()`) compare equal
  through the canonical type.
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
  unless the decl-specifier is a lone `auto`.
- **Prototype scope:** the parser brackets each
  parameter-declaration-clause with `act_on_start_prototype`,
  `act_on_prototype_param` and `act_on_end_prototype`, which keep
  the parameter names in `prototype_params`. Parameters are not
  declarations before their function is, so an unqualified name
  that matches one inside the declarator (a trailing return type, a
  later parameter, a default argument) is reported as "not supported
  yet" rather than looked up in an outer scope.
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
  every redeclaration gets the deduced type, and a use before
  deduction is `function 'f' with deduced return type cannot be used
  before it is defined`.
- Braced initializers (`auto x = {1}`, `std::initializer_list`) are
  §10.

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
  - `using_if_exists` (`__attribute__((using_if_exists))` or
    `[[clang::using_if_exists]]`; other `clang::` attributes are still
    ignored), on using-declarations (§4.8)
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
- **Gaps:** the function's parameters are not yet in scope in the
  specification (they are declared after the declarator is complete);
  deferred specifications of member functions, `__attribute__((nothrow))`
  and the implicit specifications of special members are M3.

### 4.8 Using-declarations, `using enum`, namespace aliases (`using.rs`)
- **Using-declarators** ([namespace.udecl]):
  `act_on_using_declarator(using_loc, qual, name, name_loc, if_exists)`
  handles one declarator of `using a::x, b::y;`.
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
    (`check_using_shadow_conflict`, from variable, function and typedef
    declarations) reports `declaration conflicts with target of using
    declaration already in scope`, and is marked invalid.
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
    - unary and binary operators, including compound assignment (its
      computation type promotes the left operand's type, bit-fields
      by width, and an unscoped-enumeration left operand is then an
      error: sema-conv.md §6)
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
| implicit calls: constructors, destructors, conversion functions, allocation functions | M3 (none exist in M2) |
| `__attribute__((nothrow))` on the callee | M3 (not parsed) |
| a dependent operand (value-dependent `CXXNoexceptExpr`) | M3 (no templates) |

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
  ([class.copy.elision]/3).
- In `?:`, a throw operand gives the other operand's type and value
  category ([expr.cond]/2).
- clang's exemption for system headers is not implemented. The Oven
  treats a `throw` it reaches as not a constant expression; codegen
  does not lower it yet.

### 5.6 `typeid` (`rtti.rs`)
`act_on_typeid(Ok(expr) | Err(type), loc, rparen)` ([expr.typeid]):
an lvalue of type `const std::type_info`.
- `std::type_info` must have been declared (clang's
  `you need to include <typeinfo> before using the 'typeid' operator`),
  and RTTI enabled.
- References and top-level cv are ignored; a cv-qualified expression
  operand gets a `NoOp` cast to its unqualified type, as in clang.
- A class type must be complete.
- The operand is always unevaluated: M2 has no polymorphic classes, so
  the node (`typeid_expr`, sema-class.md §5.1) is built with
  `potentially_evaluated` false. The Oven and codegen do not evaluate
  `typeid` yet.

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
  - In a default argument, the parser reports
    `default argument may not use a GNU statement expression`.
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

  Class and array literals (temporary materialization) and literals
  at namespace scope (constant elements, `ConstantExpr` wrapping) are
  reported as not supported yet.

### 5.9 New- and delete-expressions (`new_delete.rs`, WP-061a)
clang's `ActOnCXXNew`/`BuildCXXNew` and `ActOnCXXDelete` for non-class
and aggregate element types.
- **Implicit global operators** ([basic.stc.dynamic.general]/2):
  `declare_global_new_delete` runs once, the first time a new- or
  delete-expression or a declaration of a global `operator new`/`delete`
  needs it (clang's `DeclareGlobalNewDelete`). It declares, as implicit
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
- Gaps: `-fno-sized-deallocation` only removes the macro (Sema always
  selects the sized form, clang's C++14+ default); `-Wmismatched-new-delete`
  is not reported; a non-constant bound in a parenthesized type-id is a
  plain warning (clang's is an extension warning, so an error under
  `-pedantic-errors`); `__hot_cold_t` overloads are not recognized;
  default arguments of a user-declared allocation function are not
  used (every parameter must be matched by an argument).

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

### 6.1 Braced lists in expressions and explicit type conversions (`forms.rs`)
- **Call arguments** `f({...})` ([expr.call]/7):
  - `act_on_braced_arg` returns a `void` `InitListExpr` placeholder,
    kept in `braced_args`.
  - `act_on_call` replaces it by the list-initialization of the
    parameter.
  - Viability follows [over.ics.list]: a scalar takes at most one
    unbraced, convertible element; an aggregate at most as many
    elements as brace elision reaches. A non-viable list is clang's
    `cannot convert initializer list argument to 'T'` candidate note.
  - A list for the ellipsis of a variadic function is an error.
- **`return {...};`** ([stmt.return]/2, `act_on_return_list`):
  copy-list-initialization of the returned object.
- **`x = {...}`** ([expr.assign]/8, `act_on_assign_list`): the list
  initializes a temporary of the left operand's type, which is then
  assigned.
- **Other operators:** a braced list as their operand is ill-formed
  (`act_on_misplaced_init_list`).
- **Functional casts** ([expr.type.conv]):
  - `T()` (`act_on_value_init_cast`) value-initializes; `void()` is
    a `void` prvalue.
  - `T{...}` (`act_on_list_cast`) is direct-list-initialization.
  - `T(a, b)` (`act_on_paren_list_cast`) is ill-formed for a scalar.
  - Class, array and reference `T` need temporaries and are
    unsupported (`temporary_unsupported`) until classes have
    constructors (WP-065).

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
    `in_immediate_context()` reports it. Nothing consumes it yet: the
    immediate-invocation checks it relaxes are M3.
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

## 8. Constant evaluation (`oven_bridge.rs`)

Sema does no constant evaluation of its own: every constant it needs
comes from the Oven (`kiln_oven`, spec/oven.md; "Integration" there
covers the Oven's side). `constfold.rs` keeps only integer helpers
(`int_info`, `wrap`, `fits`). The M2 `Folder`, its tree-walking `Eval`
interpreter and `aggval` were removed by WP-054.

### 8.1 One Oven per translation unit
- `Sema` owns one `Oven`. Compiled bytecode, resolved globals and the
  memo cache are shared by every evaluation in the TU.
- `oven_bridge.rs` implements `SemaBridge` over the `Sema`'s AST,
  target and diagnostics engine for the duration of one call:
  - `ensure_body` returns the body of the function's definition
    (there are no templates in M2);
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
- **Classes:** member functions, special members, `this`, access
  control, bases and virtual functions. Design: [sema-class.md](sema-class.md).
- **Overloading:** overload resolution beyond arity, user-defined
  conversions, operator overloading.
- **Templates:** templates, concepts, packs; `auto` and `decltype`
  beyond the M2 basics. Design: spec/sema-templates.md.
- **Other language features:** lambdas, exceptions (`try`/`catch`
  and lowering `throw`; exception specifications, the `noexcept`
  operator and `throw` are §4.7, §5.3 and §5.5), coroutines,
  reflection, contracts.
- **WP-064 follow-ups:**
  - prototype scope (§4.3)
  - `dynamic_cast` and polymorphic `typeid`
  - class and array temporaries (§6.1) and compound literals (§5.8)
  - `-Wconstant-conversion` and `-Wliteral-conversion` alongside
    narrowing errors in list-initialization
  - statement expressions: `-Wgnu-statement-expression`, jumps into
    one, and class-glvalue results
- **Constant evaluation** (what the Oven supports in M2 is in
  spec/oven.md):
  - `consteval` and immediate invocations (the immediate function
    context of `if consteval` is tracked, §7, but not consulted)
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
