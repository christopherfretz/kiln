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
  - `using namespace`
  - `static_assert`
- **Attributes:** standard and GNU attributes (§6.2).
- **Statements:** every C control-flow statement, with C++
  init-statements and condition declarations.
- **Expressions:**
  - every built-in operator, with clang's precedence
  - calls, subscripts, `.`, `->`
  - `sizeof`, `alignof`, and `sizeof...` (diagnosed: M2 has no packs)
  - C-style, functional, `static_cast`, `reinterpret_cast` and
    `const_cast`
  - `nullptr`, `true`, `false`
  - string-literal concatenation
  - literals; user-defined literals are diagnosed because M2 has no
    literal operators

### 1.3 Entry points
| fn | grammar |
|---|---|
| `parse_file(sm, file, diags, target, opts) -> Ast` | a whole translation unit |
| `Parser::parse_translation_unit` | translation-unit ([basic.link]/1) |
| `Parser::parse_external_declaration` | declaration ([dcl.pre]) |
| `Parser::parse_declarator` (crate) | declarator ([dcl.decl]) |
| `Parser::parse_expression` / `parse_assignment_expr` / `parse_constant_expr` / `parse_expr_prec(min)` | expression / assignment-expression / constant-expression / binary operators at precedence ≥ `min` |
| `Parser::parse_statement` | statement ([stmt.pre]) |

`parse_file` lives in `kiln_parse`, not in the driver, so tests and the
driver share one path. It lexes the main file (§2), maps lexer
diagnostics (§8), builds a `Sema::for_target`, parses, and returns the
AST. The driver adds `-fsyntax-only -fdump-ast` on top (§7).
`parse_tu` does the same with the driver's `ConstexprLimits` (the
`-fconstexpr-*` budget and backtrace limit, applied to Sema's Oven) and
returns a `ParsedTu { ast, oven }`, so the driver can write the
`-fconstexpr-report` from the Oven's statistics (spec/oven.md
"Integration"). `parse_file` is `parse_tu` with the default limits, keeping
only the AST.

## 2. Token buffer (`tokens.rs`)
- `TokenBuffer::lex` lexes the whole main file up front, so lookahead is
  an index (`peek(n)`, `tok_at(i)`).
- Comments are dropped. Alternative tokens become their punctuators.
- Literal spellings are kept in a parallel `spellings` vector.
- Preprocessing directives are skipped, and recorded in `skipped`:
  - `#pragma` is accepted.
  - `#pragma pack` and every other directive produce
    `"... is not supported yet"`.
- This is a stopgap until the preprocessor (kiln_pp, WP-012, now
  merged) feeds the parser. The switch belongs to the driver work
  (WP-014). See §9.

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
    bound, function with parameters and variadic flag
  - the bit-width, attributes, and `init_style`

  The parser peeks at `=`, `(` or `{` to set `init_style` *before* the
  declaration exists, because Sema needs it to create the `VarDecl`.
  Sema builds the type from the last chunk outwards ([dcl.meaning]).
- **`parse_simple_declaration`** handles the decl-specifiers, then
  init-declarators. Each one goes to `Sema::act_on_declarator`, then
  `parse_initializer`/`act_on_init` or `act_on_uninit`. For a
  function definition the body goes through `act_on_start_function`
  and `act_on_finish_function`.
  - A declaration with no declarator (`struct S;`,
    `union { int a; };`) goes to `act_on_decl_spec_only`.
  - That call returns the implicit object of an anonymous union, if
    any, which then joins the `DeclStmt`.
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
    and `(type-id) cast-expression` when the parenthesized tokens name
    a type (`scan_name` → `NameClass::Type`, or a type keyword).
  - Then postfix operators: calls, subscripts, `.`, `->`, `++`, `--`.
  - Then primary expressions:
    - literals and `this`-less id-expressions
    - qualified names `N::x`
    - `( expr )`
    - functional casts `T(x)`/`T{x}` for a simple type specifier or
      type name
    - named casts `static_cast<T>(e)` etc.
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
- **Unsupported:** case ranges, computed goto, statement expressions,
  range-based `for`, and `if constexpr`/`consteval` are "not supported
  yet".

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
- The driver calls `parse_file` and prints diagnostics.
- With `-fdump-ast` it prints `kiln_ast::dump` (spec/ast-dump.md).
- The exit code is 1 if any error was reported.

## 8. Lexer-diagnostic stopgap (`lexdiag.rs`)
- Until the preprocessor feeds the parser, `lex_diag` maps
  `kiln_lex::LexDiagKind` to `kiln_diag` diagnostics:
  - Kinds that have an `lex_*` id of their own use it.
  - Other errors use `lex_literal_error`, and other warnings
    `lex_literal_warning`, both with clang's wording.
  - Extensions are not reported.
- kiln_pp (WP-012) now owns the complete mapping. When the parser
  switches to kiln_pp tokens, this module is deleted.

## 9. To be implemented (M3+)
- **Inputs:**
  - tokens from kiln_pp instead of the raw-lexer buffer (§2), which
    brings macros, `#include` and `#if`
  - `#pragma pack`
- **Tentative parsing:**
  - `T(x);`, `T(x)` in parameter lists, and `(T)(x)` disambiguation
    ([stmt.ambig], [dcl.ambig.res])
  - `T(x)` with several arguments
- **Classes:**
  - member functions, access specifiers, base classes
  - static data members, default member initializers
  - constructors, destructors, special members, friends
  - `this`
  - operator function names, conversion functions
  - deleted and defaulted functions
- **Templates:** declarations, template-ids, `typename`, `template`,
  packs, concepts, requires-clauses.
- **Other expressions:**
  - lambdas, `new`/`delete`, `throw`
  - `typeid`, `dynamic_cast`
  - compound literals, statement expressions, the GNU `?:` and
    address-of-label operators
  - `T()`, `T{…}` and `T(a, b)` temporaries
  - fold expressions
  - braced-init-list operands and arguments
  - pointer-to-member operators on classes
  - reflection (`^^`, `[: :]`)
  - `co_await`/`co_yield`
- **Statements:** range-based `for`, `if constexpr`/`consteval`,
  `try`/`catch`, `co_return`, contracts.
- **Other declarations:**
  - pointers to members, asm labels
  - exception specifications
  - default arguments
  - trailing return types
  - `auto` deduction beyond `auto x = e`
  - namespace aliases, `using` declarations, `using enum`
  - `asm`, modules, `__declspec`

## 10. Tests
- **Per-rule tests** in `crates/kiln_parse/tests/`, each tagged with
  its standard section:
  - `decl.rs`: [dcl.*], [class.*], [namespace.*], [basic.def.odr],
    [basic.link]
  - `expr.rs`: [expr.*], [lex.string], [lex.ext]
  - `stmt.rs`: [stmt.*]
  - `sema.rs`: rules that need the parser in front of Sema
  - `common/mod.rs` provides `parse(src) -> Parsed` with
    `assert_clean`, `assert_diag(substr)` and `assert_dump(substr)`.
- **Conformance tests:** `diag:` tests under
  `tests/conformance/{dcl.*,expr.*,stmt.*,basic.*,class.*,namespace.*}/`.
  They pass with both `cargo xtask test` and
  `cargo xtask test --compiler clang++-19`.
- **AST goldens:** `tests/conformance/ast/` (spec/ast-dump.md),
  generated with `cargo xtask clang-ast`.
