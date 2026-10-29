# spec: kiln_parse

Status: **implemented for M2** (WP-048). Normative for `kiln_parse`.
Everything outside the M2 subset (§1.2) is diagnosed as
`"{what} is not supported yet"` and recovered from; §9 lists what is
still to be implemented.

Standard: [lex.key], [gram] (the grammar summary), [dcl], [expr], [stmt],
[namespace], [class.mem], [dcl.attr].
Reference: `clang++-19 --target=x86_64-linux-gnu -std=c++26
-fsyntax-only` for diagnostics (wording, location, notes), and
`cargo xtask clang-ast` (spec/ast-dump.md) for the AST.

## 1. Scope and entry points

### 1.1 The Parser/Sema split
The parser owns the grammar and the token stream; `kiln_sema::Sema`
(spec/sema.md) owns scopes, name lookup, declarations, types and the
typed AST. This is clang's split. The parser never builds an AST node
directly. It calls `Sema::act_on_*` and keeps only the returned ids.

The one thing the parser asks Sema during parsing is how a name
classifies. `Sema::classify_name` returns a type, namespace, value or
undeclared, which resolves the C++ declaration/expression ambiguities.

### 1.2 The M2 subset
- **Types:** fundamental types, cv, pointers, lvalue and rvalue
  references (as declarations), arrays, functions (including
  variadic), enumerations (scoped, unscoped, fixed), structs and unions
  with bit-fields, anonymous unions and structs, typedef and `using`
  aliases.
- **Declarations:**
  - functions (declarations and definitions)
  - variables (global, local, `static`, `extern`, `constexpr`, `const`)
  - `extern "C"`/`"C++"` blocks and declarations
  - namespaces (named, unnamed, `inline`, nested `A::B`)
  - qualified declarator-ids
  - exception specifications: `noexcept`, `noexcept(e)`, `throw()`
    (and `throw(type-ids)`, diagnosed)
  - `using namespace`, using-declarations, `using enum`, namespace
    aliases (WP-062)
  - `static_assert`
  - default arguments, trailing return types, `decltype(e)` and
    `decltype(auto)`, `auto` with pointer/reference declarators and
    deduced return types (WP-064)
- **Attributes:** standard and GNU attributes (§6.2).
- **Statements:** every C control-flow statement, with C++
  init-statements and condition declarations; `if constexpr`,
  `if consteval` and `if !consteval`.
- **Expressions:**
  - every built-in operator, with clang's precedence
  - calls, subscripts, `.`, `->`
  - `sizeof`, `alignof`, and `sizeof...` (diagnosed: M2 has no packs)
  - the `noexcept(e)` operator
  - C-style, functional (`T(e)`, `T()`, `T{...}`), `static_cast`,
    `reinterpret_cast`, `const_cast` and `dynamic_cast` (parsed; Sema
    reports it unsupported)
  - `throw`, `typeid`
  - braced-init-list call arguments, return values and assignment
    operands
  - GNU extensions: `x ?: y`, statement expressions `({ ... })`,
    `__builtin_offsetof`, scalar compound literals `(T){...}`
    (spec/sema.md §6.10)
  - `nullptr`, `true`, `false`
  - string-literal concatenation
  - literals; user-defined literals are diagnosed because M2 has no
    literal operators

### 1.3 Entry points
| fn | grammar |
|---|---|
| `parse_file(sm, interner, file, diags, target, opts) -> Ast` | a whole translation unit, preprocessed with default `PpOptions` (no include paths), default `ConstexprLimits` |
| `parse_tu(sm, interner, file, diags, target, opts, limits) -> ParsedTu` | the same, with explicit constant-evaluation limits; also returns the Oven |
| `parse_preprocessed(pp, interner, diags, target, opts, limits) -> (ParsedTu, Preprocessor)` | a whole translation unit read from a configured preprocessor (the driver's path) |
| `Parser::parse_translation_unit` | translation-unit ([basic.link]/1) |
| `Parser::parse_external_declaration` | declaration ([dcl.pre]) |
| `Parser::parse_declarator` (crate) | declarator ([dcl.decl]) |
| `Parser::parse_expression` / `parse_assignment_expr` / `parse_constant_expr` / `parse_expr_prec(min)` | expression / assignment-expression / constant-expression / binary operators at precedence ≥ `min` |
| `Parser::parse_statement` | statement ([stmt.pre]) |

**Interface change (WP-055).** All three entry points take the
`Arc<Interner>` the preprocessor and the AST share. The caller creates
it before the `DiagEngine`, because the engine's lifetime is invariant
and the preprocessor borrows both. `parse_preprocessed` asserts that
`interner` is the preprocessor's interner, and returns the preprocessor
so the caller can read its state afterwards (dependencies, `#pragma
once` files).

`ParseOptions { std, keep_tokens }`:
- `std` is the lexer standard, `Cxx26` by default. The driver passes
  `-std=`.
- `keep_tokens` keeps the whole token stream in memory instead of the
  lookahead window (§2.1). It exists for measurement and tests; the
  parse is identical.

`ConstexprLimits { budget, backtrace_limit }` are the driver's
`-fconstexpr-*` limits (WP-054; spec/oven.md "Integration"), applied to
Sema's Oven. The default is clang's step, depth and backtrace limits and
kiln's 256 MiB memory budget.

`parse_preprocessed` builds a `Sema::for_target` with the limits, wraps
the preprocessor in a `TokenSource` (§2), parses, and returns a
`ParsedTu { ast, oven }` with the preprocessor. The Oven's statistics
feed `-fconstexpr-report`. The driver configures the preprocessor
exactly as `-E` does, calls `parse_preprocessed`, and writes the report
(§7). `parse_tu` builds a `Preprocessor` for `file` with
`PpOptions::new(opts.std)` and calls `parse_preprocessed`. `parse_file`
is `parse_tu` with the default limits, keeping only the AST; unit tests
use it. All three live in `kiln_parse`, so tests and the driver share
one path.

## 2. Token source (`tokens.rs`, `pragma.rs`)

The parser reads phase-4 tokens ([lex.phases]/1.4: macro-expanded, with
directives executed) from `kiln_pp::Preprocessor::next_item`. This is
clang's `Parser` over `Preprocessor::Lex`.

### 2.1 The lookahead window
- `TokenSource` holds the tokens from a little behind the parser's
  position to the furthest token looked at. Lookahead is by absolute
  index (`tok_at(i)`), so arbitrary-distance scans such as `scan_for`
  and `at_constructor_declaration` work unchanged; the window grows to
  cover them.
- `bump` releases consumed tokens. The source keeps `KEEP_BEHIND` = 8
  tokens behind the position (the parser reads back at most one, for
  `prev_end`), and drops consumed tokens in batches of 256 so the cost
  is amortized.
- `ParseOptions::keep_tokens` disables release. Measured with
  `-fsyntax-only -ferror-limit=0` (release build, median of 5): the
  window and the whole buffer take the same time (`headers`: 2.40 s vs
  2.37 s; `xmacro`: 3.84 s vs 3.87 s), and the window uses 21% less
  (`headers`, 99 MB vs 125 MB) and 11% less (`xmacro`, 336 MB vs
  378 MB) peak RSS. The window is the default.
- Parsing adds no measurable time over preprocessing: `-fsyntax-only`
  runs in 2.10 s on `headers` (`-E -o /dev/null`: 2.21 s) and 3.69 s on
  `xmacro` (`-E`: 3.81 s), because `-E` spends more time formatting its
  output than the parser spends. Until templates are parsed, much of
  `headers` is skipped as unsupported, so re-measure when M3 lands.
- After a fatal error (`'x' file not found`, `-ferror-limit` reached),
  the source returns `Eof`, so parsing stops, as in clang.
- **Tentative parsing** (`tentative.rs`, WP-064) never rewinds: it is a
  side-effect-free scanner over absolute token indices (clang's
  `ParseTentative.cpp` without `TentativeParsingAction`). It classifies
  names through Sema lookup but consumes, diagnoses and declares
  nothing; identifiers its own declarators would declare are tracked
  locally (clang's `TentativelyDeclaredIdentifiers`). A scan grows the
  window to the furthest token it reads, which `KEEP_BEHIND` does not
  bound because nothing behind the position is read. Entry points:
  - `is_cxx_declaration_statement(allow_for_range)` ([stmt.ambig]),
  - `is_cxx_condition_declaration()` (conditions),
  - `is_cxx_type_id(ctx)` / `is_cxx_type_id_at(i, ctx) -> (bool, bool)`
    ([dcl.ambig.res]/2; the second flag says the decision was
    ambiguous), with `TypeIdContext` as clang's
    `TentativeCXXTypeIdContext`,
  - `is_cxx_function_declarator(name) -> (bool, bool)`
    ([dcl.ambig.res]/1; the ambiguity flag feeds `-Wvexing-parse`).

### 2.2 Tokens
- **Keywords** are classified by the parser (`keywords.rs`, one hash
  lookup on the identifier's symbol). The preprocessor delivers them as
  identifiers.
- **Alternative tokens** ([lex.digraph]/2: `and`, `bitor`, `not_eq`,
  ...) become their punctuators in the token source, including when
  they come from a macro expansion. `#` still sees them as identifiers
  (`#and` is `"and"`), because stringizing happens in the preprocessor.
- **Spelling** comes from the source manager (`kiln_lex::spelling` of
  the token's span). Spans of macro-expanded tokens are expansion
  locations, so diagnostics carry "expanded from macro" notes.
- **Literal diagnostics** (invalid escapes, out-of-range characters,
  unevaluated-string prefixes, ...) are mapped by
  `kiln_pp::literal_diags`, the mapping `-E` and `#if` use. The parser
  reports them when it converts a literal (§5). (WP-055 deleted the
  `lexdiag.rs` stopgap.)
- Diagnostics in included files, and everything else the preprocessor
  reports, go through the shared `DiagEngine`.

### 2.3 Pragmas and module directives
The preprocessor handles `#pragma once`, `push_macro`/`pop_macro`,
`poison`, `system_header`, `GCC warning`/`error`, `message` and
`_Pragma` (spec/pp.md). It passes every other pragma to the parser as a
`Marker::Pragma` with its tokens.

The token source classifies each pragma as soon as it reads the marker
(`pragma::classify`), so syntax warnings are reported in the place of
clang's pragma handlers. Pragmas with a semantic effect become an
`Event`, keyed by the index of the next token. The parser acts on events
when it reaches that token (`handle_events`, as for clang's
`annot_pragma_*` tokens).

| Pragma | Handling |
|---|---|
| unknown (also unknown `GCC x` / `clang x`, empty `#pragma`) | `unknown pragma ignored` [-Wunknown-pragmas], at the first unrecognized token |
| `STDC FP_CONTRACT`, `FENV_ACCESS`, `CX_LIMITED_RANGE`, `FENV_ROUND` | accepted, no effect |
| `STDC <other>` | `unknown pragma in STDC namespace` [-Wunknown-pragmas] |
| `GCC visibility push(v)` / `pop` | syntax warnings [-Wignored-pragmas] (missing `(`/`)`, `expected identifier`, extra tokens); `Event::Visibility`. An unknown `v` gets `unknown visibility 'v'` [-Wignored-attributes] and is not pushed; an unmatched `pop` is an error. The visibility itself is not applied yet (M3). |
| `pack ...` | missing `(` gets `missing '(' after '#pragma pack' - ignoring`. Otherwise `Event::PragmaPack`; Sema records `(loc, args)` in `Sema::pragma_packs` and reports `'#pragma pack' is not supported yet` (layout does not apply it yet). |
| `GCC diagnostic ...`, `clang diagnostic`, `clang loop`, `clang fp`, `clang optimize`, `clang max_tokens_*`, `clang __debug` | accepted, ignored. **Not applied:** `clang diagnostic ignored` does not silence anything. |
| `comment`, `float_control`, `unused`, `omp`, `unroll`, `nounroll`, `unroll_and_jam`, `nounroll_and_jam` | accepted, ignored |
| `weak`, `redefine_extname`, `align`, `options`, `ms_struct`, `clang attribute`, `clang section` | `Event::UnsupportedPragma`: `'#pragma <name>' is not supported yet` |

A `module` or `import` directive ([cpp.module], [cpp.import]) is
removed from the stream through its `;`. The parser reports `a module
import is not supported yet` or `a module declaration is not supported
yet` and continues with the next declaration.

**Known divergence: ordering.** The preprocessor processes all the
directives up to the next token before it returns their markers. So a
preprocessor-level warning (for example `#pragma once in main file` from
`_Pragma`) can come before the syntax warnings of an earlier `#pragma`.
Locations and texts match clang; only the order differs, and the test
harness ignores order.

## 3. Parser core (`parser.rs`)
- **Token access:**
  - `tok`, `peek`, `bump`, `at(p)`, `at_kw(k)`, `eat`, `eat_kw`
  - `ident()` returns a non-keyword identifier.
  - Keywords are identifiers that `keywords.rs` maps to `Kw` with one
    hash lookup. The table includes the GNU spellings (`__inline__`,
    `__attribute__`, ...).
- **Bracket depth:** `bump` tracks `(`, `[` and `{` nesting, like
  clang's `ParenCount`/`BracketCount`/`BraceCount`.
- **Expected tokens:**
  - `expect(p)` reports `expected 'p'` at the end of the previous
    token (`expected_loc`), as clang does.
  - `expect_closing(close, open, open_loc)` also adds the
    `to match this 'open'` note.
- **Recovery:**
  - `skip_until(stops, consume, stop_at_semi)` is clang's `SkipUntil`.
    It balances brackets and never skips past a closer that belongs to
    an outer bracket.
  - `skip_to_semi` skips to the end of the current declaration or
    statement.
  - Diagnostics use clang's wording and locations, for example:
    - `expected ';' after top level declarator`
    - `expected ';' at end of declaration`
    - `expected expression`
    - `expected unqualified-id`
- **Expression results:** `PExpr = Option<ExprId>`. `None` means a
  *syntax* error was reported and the caller must recover. A
  *semantic* error produces an error expression (`act_on_error_expr`),
  so parsing continues normally.
- **Name scanning:**
  - `scan_name(i)` classifies a possibly qualified name starting at
    token `i` without consuming it.
  - `scan_for(targets)` finds the first target punctuator at depth 0.
  - `parse_nested_name` consumes `A::B::`.

## 4. Declarations (`decl.rs`)
- **`parse_decl_specifiers`** fills a `kiln_sema::DeclSpec`:
  - storage class, `typedef`, `inline`, `constexpr`, `constinit`,
    `thread_local`, `friend`
  - type specifiers, cv-qualifiers, sign/width keywords
  - a type name (`NameClass::Type`) or tag specifier
  - attributes

  Sema checks the combination when it converts the DeclSpec to a type
  (`[dcl.type.simple]`, `type_from_declspec_at`).
- **`parse_declarator`** fills a `Declarator`:
  - the name, its qualifier and its location
  - `chunks`, innermost first: pointer/reference with cv, array with
    bound, function with parameters, variadic flag and exception
    specification
  - the bit-width, attributes, and `init_style`

  The parser peeks at `=`, `(` or `{` to set `init_style` *before* the
  declaration exists, because Sema needs it to create the `VarDecl`.
  Sema builds the type from the last chunk outwards ([dcl.meaning]).
  - A `(` after the declarator-id is a parameter list or a direct
    initializer by `is_cxx_function_declarator`; when the choice was
    ambiguous the function chunk records it and Sema warns
    `-Wvexing-parse` (`parentheses were disambiguated as a function
    declaration` / `empty parentheses interpreted as a function
    declaration`, with clang's fix-it notes).
  - Parameters: `= initializer-clause` default arguments go to
    `act_on_param_default_init` (a default argument starting with `({`
    is `err_stmt_expr_in_default_arg`, skipped to `,`/`)`). The
    parameter-declaration-clause brackets `act_on_start_prototype` /
    `act_on_prototype_param` / `act_on_end_prototype` (closed after the
    trailing return type): naming a parameter inside its own
    declarator is reported "not supported yet" (no prototype scope;
    parameters become declarations with their function).
  - `-> type-id` after the exception specification is a trailing
    return type (`Chunk::Function::trailing`), required with a
    leading `auto` and diagnosed otherwise by Sema.
  - `decltype ( expression )` and `decltype(auto)` are type specifiers;
    the operand is unevaluated.
- **Exception specifications** (`parse_exception_spec`, after the
  function parameters, [except.spec]/1): at most one per declarator.
  - `noexcept` is `ExcSpec::BasicNoexcept`.
  - `noexcept ( constant-expression )` goes to
    `act_on_noexcept_spec(e, loc, start)` (spec/sema.md §4.7).
  - `throw ( [type-id-list] )` goes to `act_on_dynamic_exception_spec`;
    a missing `(` is `expected '(' after 'throw'`, and a pack expansion
    `...` in the list is "not supported yet".
  - A second specification is not consumed: the declarator ends and
    the next token is diagnosed by the caller.
- **`parse_simple_declaration`** handles the decl-specifiers, then
  init-declarators. Each one goes to `Sema::act_on_declarator`, then
  `parse_initializer`/`act_on_init` or `act_on_uninit`. For a
  function definition the body goes through `act_on_start_function`
  and `act_on_finish_function`.
  - A declaration with no declarator (`struct S;`,
    `union { int a; };`) goes to `act_on_decl_spec_only`.
  - That call returns the implicit object of an anonymous union, if
    any, which then joins the `DeclStmt`.
- **GNU asm labels** (`parse_asm_label`, clang's
  `ParseSimpleAsm(ForAsmLabel)`): `asm`/`__asm`/`__asm__` `(`
  string-literal... `)` after a declarator, before its attributes and
  initializer. Adjacent literals concatenate ([lex.string]/7); the
  result is stored in `Declarator::asm_label` (the literal's `ExprId`
  and the span of its tokens) for Sema (spec/sema.md §4.2).
  - A missing `(` is `expected '(' after 'asm'`; a missing literal is
    `expected string literal in 'asm'`; a wide, Unicode or empty literal
    is `cannot use {unicode|wide|an empty} string literal in 'asm'`.
  - In a simple declaration a malformed label drops the declarator (the
    parser skips to the `;`, clang's `ParseAsmAttributesAfterDeclarator`);
    in a member declaration it skips to the next `,`.
  - A function declarator with a label is never a definition: a
    following `{` is parsed as an initializer, which Sema rejects
    (`illegal initializer`).
  - A malformed braced initializer without `=` (`T x{…}`) is skipped
    through its `}` and the declaration continues, as clang's
    `ParseBraceInitializer` does.
- **Tags** (`parse_tag_specifier`):
  - `act_on_tag(kind, name, TagUse, …)`. `TagUse` is reference,
    declaration or definition, decided by the next token, as in
    [dcl.type.elab].
  - Then `act_on_tag_attrs` for the class-head or enum-head attributes.
  - Then the body:
    - `parse_record_body` holds member declarations, bit-fields and
      nested anonymous records.
      `act_on_finish_record_definition(r, then_semi)` is told whether a
      `;` follows, which decides whether an unnamed class is anonymous.
    - `parse_enum_body` holds enumerators, with an optional fixed
      underlying type and `enum class`/`enum struct`.
- **Namespaces:**
  - `namespace N { }`, `inline namespace`, `namespace A::B { }`, and
    unnamed namespaces (§sema 4.6)
  - `extern "C" { }` and `extern "C" decl` (`parse_linkage_spec`)
  - `using namespace N;`
- **Using-declarations and aliases** (`parse_using`, spec/sema.md §4.8):
  - `using a::x, ::b::y;`: each declarator is a nested-name-specifier
    (`parse_nested_name_nns`, which also builds the specifier as
    written) and an identifier, passed to
    `Sema::act_on_using_declarator` one at a time.
    - `typename` declarators, `operator`/destructor names and pack
      expansions (`x...`) are "not supported yet" (they need classes
      with bases and templates).
    - A missing name is `expected unqualified-id`; a missing `;` is
      `expected ';' after using declaration`.
  - `[[clang::using_if_exists]]` is accepted before `using` (at
    namespace and class scope; `at_attributed_using`) and after a
    declarator. Both placements are a clang extension and warn `ISO C++
    does not allow an attribute list to appear here`, as clang does;
    GNU `__attribute__((using_if_exists))` after the declarator does
    not warn.
  - `using enum [qual] name;` classifies the name and calls
    `Sema::act_on_using_enum`. No name: `using enum requires an enum or
    typedef name`. Missing `;`: `expected ';' after using-enum
    declaration`.
  - `namespace Q = [qual] target;` at namespace and block scope calls
    `Sema::act_on_namespace_alias`. A target that is not a name is
    `expected namespace name`; a missing `;` is `expected ';' after
    namespace name`.
- **`static_assert(cond[, msg])`:** the message is optional (C++17).
- **Initializers** (`parse_initializer`, crate-visible):
  - `= expr`, `= { … }`, `( expr )` and `{ … }`
  - designated initializers `.x = …` (C++20)
  - The result is a `kiln_sema::Initializer` tree.

## 5. Expressions (`expr.rs`)
- **Binary operators** use operator-precedence parsing
  (`parse_expr_prec(min)`) with clang's `prec::Level` values, from
  comma (1) to pointer-to-member (15).
  - Assignment and `?:` are right-associative.
  - The middle operand of `?:` is a full expression.
- **Below a cast-expression**, parsing is recursive descent:
  - `parse_cast_expr` handles the unary operators, `sizeof`/`alignof`,
    `noexcept ( expression )` (`parse_noexcept_expr`: the operand is
    parsed between `enter_unevaluated`/`exit_unevaluated`, then
    `act_on_noexcept_expr`; a missing `(` is `expected '(' after
    'noexcept'`), and `(type-id) cast-expression` when
    `is_cxx_type_id_at(pos + 1, InParens)` says the parentheses hold a
    type-id. `( type-id ) { ... }` is a compound literal instead
    (`act_on_compound_literal`), followed by postfix operators as in
    clang's `ParseCompoundLiteralExpression`.
  - `throw [assignment-expression]` (the operand is optional before
    `)`, `]`, `}`, `;`, `,` and `:`) and `typeid ( type-id |
    expression )`, disambiguated with `is_cxx_type_id`; the operand of
    a type-id-shaped `typeid` is unevaluated.
  - Then postfix operators (`parse_postfix_suffix`): calls, subscripts,
    `.`, `->`, `++`, `--`. A call argument may be a braced-init-list
    (`act_on_braced_arg`, converted once the parameter type is known).
  - Then primary expressions:
    - literals and `this`-less id-expressions
    - qualified names `N::x`
    - `( expr )`, and `( { ... } )`, a GNU statement expression
      (`check_stmt_expr_allowed`, `act_on_start_stmt_expr`,
      `act_on_stmt_expr`; at namespace scope it is skipped to `)`)
    - functional casts `T(x)`, `T()`, `T{...}` and `T(a, b)` for a simple
      type specifier or type name. A type name not followed by `(` or
      `{` is `expected '(' for function-style cast or type
      construction` after a keyword or qualified name, and
      `unexpected type name 'T': expected expression` (at the name) for
      a lone identifier, as clang only annotates the latter before `(`
      or `{`.
    - named casts `static_cast<T>(e)` etc.
    - `__builtin_offsetof ( type-id , designator )`: an identifier
      followed by `.name` and `[expr]` components, each handed to
      `act_on_builtin_offsetof` as an `OffsetofComponent`. clang's
      parser errors: `expected '(' after '__builtin_offsetof'`,
      `expected ','`, `expected identifier` (skip past `)`), a missing
      `]` (reported, parsing continues) and a missing `)` (reported;
      the expression fails).
- **The GNU `x ?: y`** goes to `act_on_binary_conditional`; the middle
  operand of an ordinary `?:` is a full expression.
- **Missing closers** (`expect_closing`) are reported at the current
  token with `to match this '('`, then, unless already at a closing
  bracket, the parser skips to the closer and takes it (clang's
  `BalancedDelimiterTracker::diagnoseMissingClose`).
- **Literals:**
  - Integer, floating and character literals are classified by
    `kiln_lex::literal` and typed by `kiln_sema::literal`
    (spec/sema-conv.md §8).
  - Adjacent string literals are concatenated
    (`act_on_string_literal(pieces, unevaluated, ud_loc)`).
  - A user-defined literal suffix goes to Sema as `ud_loc`, and Sema
    reports clang's `no matching literal operator`.
- **`sizeof...(x)`:**
  - The parenthesized form goes to `act_on_sizeof_pack`, which reports
    `'x' does not refer to the name of a parameter pack`.
  - The unparenthesized form is "not supported yet".

## 6. Statements and attributes

### 6.1 Statements (`stmt.rs`)
- **Declaration or expression** ([stmt.ambig]):
  `is_declaration_statement` uses keywords, attributes and `scan_name`.
  Each case below counts as a declaration:

  | Name class | Followed by |
  |---|---|
  | type name | an identifier, `*`, `&`, `&&`, `;`, `(*`, `(&`, or a decl-specifier keyword |
  | undeclared name | an identifier |

  `T(x);` is not disambiguated yet: tentative parsing is M3.
- **Compound statements:** `parse_compound(new_scope)` pushes a `Block`
  scope. A function body reuses the `Function` scope that holds the
  parameters, so a body-level redeclaration of a parameter is diagnosed
  ([basic.scope.block]/2).
- **Control statements** push a `Control` scope for their
  init-statement and condition (`if`, `switch`, `while`, `for`).
  - Their sub-statements go through `parse_sub_statement`. That pushes
    an implicit `Block` *unless* the sub-statement is a compound
    statement, as clang does with `InnerScope`.
  - The compound's own block therefore sits directly on the `Control`
    scope, which lets Sema forbid redeclaring condition names there.
- **Conditions:**
  - `parse_paren_condition` handles `( [init-statement] condition )`.
  - A condition declaration is `decl-specifiers declarator = expr` or
    `{ … }` (`act_on_condition_var`). Without an initializer the error
    is `variable declaration in condition must have an initializer`.
- **Labels:**
  - `case`, `default`, `identifier:`
  - A label at the end of a block labels a null statement (C++23).
- **Jumps:** `goto`, `break`, `continue`, `return`.
- **`if constexpr`, `if consteval`, `if !consteval`** ([stmt.if]):
  - `if constexpr ( [init-statement] condition )` takes the ordinary
    `if` path; the condition goes to `act_on_constexpr_condition`
    instead of `act_on_condition`, and the statement is built with
    `IfKind::Constexpr`.
  - `if [!] consteval compound [else compound]`
    (`parse_if_consteval`): both substatements must be compound
    statements; otherwise `expected { after consteval` (at the
    `consteval` keyword) or `expected { after else` (at the `else`),
    and the statement is dropped. The branch run under constant
    evaluation is parsed between `enter_immediate_context` and
    `exit_immediate_context`.
  - `if constexpr` or `if !` not followed by `(` (or `consteval`) is
    `expected '(' after 'if'`; the parser skips to the next `;`.
- **Unsupported:** case ranges, computed goto, statement expressions
  and range-based `for` are "not supported yet".

### 6.2 Attributes (`attr.rs`)
- **Syntax:** `[[ ]]` (with `using ns:`), `alignas(…)`, and GNU
  `__attribute__((…))` are all collected as scope, name and arguments.
- **Arguments:**
  - Arguments are parsed as expressions only for attributes that take
    them (`takes_expr_args`): `deprecated`, `nodiscard`, `aligned`,
    `warn_unused_result`, with or without `__` around the name.
    `alignas` has its own parser.
  - Other arguments are skipped as balanced tokens.
  - `alignas(type-id)` is recognised by `scan_name`.
- **Sema's role:** Sema (`act_on_attribute`) decides what is known,
  checks the argument count and kinds, and encodes the arguments
  (spec/sema.md §4.5).
  - Unknown attributes warn `unknown attribute 'x' ignored`.
  - Known but unimplemented GNU attributes warn `'x' attribute ignored`.
  - A scoped attribute is printed by name only, as clang 19 does.
- **Placement:** the parser attaches attributes where the grammar puts
  them:
  - the decl-specifier sequence and the declarator, for the
    declaration
  - the class-head and enum-head, for the tag (`act_on_tag_attrs`)
  - before a statement, for the statement (`fallthrough`, `likely`)

## 7. Driver (`kiln -fsyntax-only [-fdump-ast]`)
- The driver builds the preprocessor exactly as `-E` does
  (`Job::build_preprocessor`: search list, predefines, `-D`/`-U`/
  `-include`, `#if` evaluation, feature queries; spec/driver.md). It
  then calls `parse_preprocessed` with the `-fconstexpr-*` limits,
  writes the `-fconstexpr-report=` from the returned Oven (spec/oven.md),
  and prints diagnostics.
- With `-fdump-ast` it prints `kiln_ast::dump` (spec/ast-dump.md) of the
  main file's top-level declarations; `-fdump-ast=all-files` adds those
  from included files, whose locations name their file.
- The exit code is 1 if any error was reported.

## 8. (Removed) lexer-diagnostic stopgap
`lexdiag.rs` is gone (WP-055). Literal diagnostics use
`kiln_pp::literal_diags` (§2.2). The `lex_*` diagnostic ids it used stay
in `kiln_diag` for now; nothing emits them.

## 9. To be implemented (M3+)
- **Pragmas:** applying `#pragma pack` to layout, `GCC visibility` to
  linkage, `clang diagnostic` to the diagnostic engine; `weak` and the
  other unsupported pragmas of §2.3.
- **Prototype scope:** parameters in scope within their own declarator
  (`decltype(n)` in a trailing return type or a later parameter;
  clang's "default argument references parameter").
- **Classes:**
  - member functions, access specifiers, base classes
  - static data members, default member initializers
  - constructors, destructors, special members, friends
  - `this`
  - operator function names, conversion functions
  - deleted and defaulted functions
- **Templates:** declarations, template-ids, `typename`, `template`,
  packs, concepts, requires-clauses. Design: spec/sema-templates.md §2.
- **Other expressions:**
  - lambdas, `new`/`delete`
  - `dynamic_cast` semantics, polymorphic `typeid`
  - class and array compound literals, compound literals at namespace
    scope, the GNU address-of-label operator
  - class `T()`, `T{…}` and `T(a, b)` temporaries
  - fold expressions
  - braced-init-list subscripts and functional-cast operands
  - pointer-to-member operators on classes
  - reflection (`^^`, `[: :]`)
  - `co_await`/`co_yield`
- **Statements:** range-based `for`, `try`/`catch`, `co_return`, contracts.
- **Other declarations:**
  - pointers to members
  - pack expansions in dynamic exception specifications
  - `auto` deduction from braced lists (`std::initializer_list`)
  - `typename`, `operator` and pack forms of using-declarators, and an
    attributed `[[...]] using` as a block-scope statement
  - `asm` declarations, modules, `__declspec`

## 10. Tests
- **Per-rule tests** in `crates/kiln_parse/tests/`, each tagged with
  its standard section:
  - `decl.rs`: [dcl.*], [class.*], [namespace.*], [basic.def.odr],
    [basic.link]
  - `expr.rs`: [expr.*], [lex.string], [lex.ext]
  - `stmt.rs`: [stmt.*]
  - `sema.rs`: rules that need the parser in front of Sema
  - `tokens.rs`: the token source: macro-expanded tokens, alternative
    tokens, pragmas, module directives, and window/whole-buffer parity
  - `common/mod.rs` provides `parse(src) -> Parsed` with
    `assert_clean`, `assert_diag(substr)` and `assert_dump(substr)`.
- **Conformance tests:** `diag:` tests under
  `tests/conformance/{dcl.*,expr.*,stmt.*,basic.*,class.*,namespace.*}/`.
  Tests of the token source are under `cpp.pragma`, `cpp.pragma.op`,
  `cpp.include` (with headers in `Inputs/`), `cpp.replace`,
  `lex.digraph`, `lex.string.uneval` and `module.import`.
  They pass with both `cargo xtask test` and
  `cargo xtask test --compiler clang++-19`.
- **AST goldens:** `tests/conformance/ast/` (spec/ast-dump.md),
  generated with `cargo xtask clang-ast`.
