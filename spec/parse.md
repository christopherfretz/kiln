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
  - `nullptr`, `true`, `false`, GNU `__null` (spec/sema.md §5.8)
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
  index (`tok_at(i)`), so arbitrary-distance scans such as
  `condition_has_init_statement` and `at_constructor_declaration` work unchanged; the window grows to
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
  Dependent qualified names follow clang's `isCXXDeclarationSpecifier`
  (WP-291). A dependent `N::m` that the scan cannot classify is a
  decl-specifier when an identifier or another decl-specifier follows it
  (`T::m const a`, `T::m b, c`: a declaration with a missing
  `typename`). Otherwise, inside `is_cxx_function_declarator`'s
  parameter clause only, it is ambiguous with clang's `InvalidAsDeclSpec`
  flag set (`param_decl_specifier`). A `)` then ends the function
  declarator only if `&`, `&&`, `[`, `{`, `=`, `->`, a cv-qualifier,
  `restrict`, `throw`, `noexcept`, `try` or a virt-specifier follows.
  Otherwise the declarator is a variable with a parenthesized
  initializer, so `void g(X<T>::m);` and `int g(X<T>::m);` are variables.
  A qualifier `X<args>::` is a dependent template-id (`template_args_dependent`,
  scanned in `scan_name_qual_at`) when `X` is a class template, an
  argument names a template parameter or a dependent entity, and `X` cannot
  name the current instantiation. Then `X<T>::m` is never classified as the
  primary template's member type. At statement or condition start such a
  name (`dependent_template_id_name`) begins an expression whatever
  follows, as in clang, so `X<T>::m d, e;` is an expression statement.
  `tests/conformance/dcl.ambig.res/dependent-qualified-param-or-init.cpp`
  covers this.
  An undeclared unqualified name in that parameter clause, not followed
  by `(`, `::` or `<` and not declared earlier in the scanned
  declaration, is ambiguous without the flag (clang's `ANK_Unresolved`),
  so `void a(u1);` and `int e(u4 *p);` are function declarations (WP-300).
  The parameter's decl-specifier parse then reports "unknown type name
  'u1'" (`unknown_type_follows`, clang `ParseImplicitInt`), the
  declaration is invalid and the function is still declared; once a
  parameter is invalid, `merge_default_args` skips "missing default
  argument" (`tests/conformance/dcl.ambig.res/undeclared-parameter-type.cpp`).
  Any decl-specifier that names no type, unqualified (`u1`) or qualified
  (`S::c`, `N::t`: "no type named 'c' in 'S'"), recovers to `int` with
  the decl-spec invalid (clang `ParseImplicitInt` / `TST_error`, WP-308),
  so the declarator keeps its shape: `S::c &r;` is an invalid `int &`
  that still reports a missing initializer (spec/sema.md "Invalid
  declarations"), and a parameter `S::c &p` takes part in overload
  resolution as `int &` ("expects an lvalue for 1st argument")
  (`tests/conformance/dcl.type.simple/unknown-type-recovers-to-int.cpp`).
  At block scope (and in a condition or for-init-statement) an
  undeclared unqualified name followed by a ptr-operator (`u5 &r;`,
  `u5 *p;`, `u5 &&r;`) is ambiguous (`undeclared_before_ptr_operator`,
  clang's `ANK_Unresolved` in `isCXXSimpleDeclaration`); the declarator
  decides, so `u5 & r + 1;` and `u5 & i;` with a declared `i` stay
  expressions, and a declaration reports "unknown type name 'u5'" and
  recovers to `int` (`unknown_type_follows` accepts a ptr-operator in
  those contexts). `const u7 &c;` is likewise "unknown type name 'u7'"
  plus the `int &` consequence, not "a type specifier is required".
  An undeclared name followed by a type-specifier keyword
  (`restrict int f();`, `foo const v = 1;`; `restrict` is no keyword in
  C++) is "unknown type name" in a namespace, class or block-scope
  declaration (clang's `ParseImplicitInt` default case; at block scope
  `undeclared_before_type_keyword` makes the statement a declaration),
  and the type, width and sign specifiers after a `type_error` are
  dropped silently (clang's `TST_error`; `DeclSpec::set_base`,
  `set_width`, `set_sign`; WP-371,
  `class.friend/restrict-atomic-decl-specifiers.cpp`).
  An unqualified typename-specifier (`typename T`, `typename Ts...[0]`;
  [temp.res.general]/3) is "expected a qualified name after 'typename'"
  and recovers as clang does, by context (`parse_typename_specifier_in`,
  WP-371): a block declaration or a type-id consumes the name and its
  pack-index-specifier as an erroneous type; a namespace-scope or member
  declaration with a pack index ends there ("expected unqualified-id" /
  "expected member name or ';' after declaration specifiers" at the name,
  skipped to the `;`); an alias-declaration whose type-id ends with the
  pack index is "expected ';' after alias declaration" at its end
  (`typename_pack_index`); in a parameter with a pack index, or in the
  first parameter of a namespace- or block-scope declarator (which clang
  parses tentatively first, so it reports the name twice), the
  parameter list ends at the name (`param_abort`: "expected ')'", the
  declarator invalid). A type-requirement `typename Ts...[0];` is not a
  type-name and takes the same diagnostic
  (`temp.res.general/typename-unqualified-pack-index.cpp`).
  Once a decl-specifier names no type (`DeclSpec::type_error`), the
  later declarators of the group are skipped up to the `;`, as clang's
  `ParseDeclGroup`, so `u5 &x1, &x2;` and `S::c &x3, &x4;` report once
  (WP-318, `tests/conformance/stmt.ambig/unknown-type-before-ptr-operator.cpp`);
  `X<int>::t f, &g;` with `X` undefined reports the undefined template
  once for the declaration, not per declarator (clang's second print is
  the per-declaration repeat of spec/diag.md §7.4.1, WP-330).
  A qualified name that its non-dependent scope does not declare,
  followed by an identifier (`S::nom x`), is not a type in a type-id or
  a declaration-specifier scan (clang returns `False` for an
  `ANK_Unresolved` `annot_cxxscope` unless `InvalidAsDeclSpec` is
  tracked), so `(S::nom x)` is a parenthesized expression, not a cast;
  a statement or a parameter still weighs it as ambiguous (above).
  A member-pointer ptr-operator (`member_pointer_end`) is scanned as
  `::`? (`template`? name (`<args>`)? `::`)+ `*`: each component is
  classified in the context of the previous one; template arguments
  are skipped (`skip_template_args`) only after `template` or a name
  that classifies as a class template, so `x<1>::y` with a variable `x`
  stays an expression; dependent components keep the scan going.

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

### 2.4 Saved tokens and replay (delayed parsing)
Complete-class contexts ([class.mem.general]/7) are parsed after the
outermost class is complete, so their tokens are saved and re-parsed
(clang's `CachedTokens` and `LateParsedDeclaration`s; §4.1).
- **Capture** (`class.rs`) moves tokens out of the stream into a
  `LateToks { toks, end }`, `end` being the location of the token that
  ended the range:
  - `capture_until(stops, out, stop_at_semi, consume_final)` is clang's
    `ConsumeAndStoreUntil`: balanced over `()`, `[]`, `{}`; it fails at
    end of input, at a `;` when asked, or at a closer of a bracket opened
    before the capture.
  - `capture_initializer(kind)` is `ConsumeAndStoreInitializer` for a
    default argument (ends at `,` or `)`) or a default member initializer
    (ends at `,` or `;`). A `?:` keeps its `:` (`capture_conditional`),
    and `operator,`-style names are taken whole. clang's
    template-argument heuristics are implemented (WP-155): a `,` after an
    unclosed `<` ends the initializer only if the tokens after it form an
    init-declarator-list followed by `;` (default member initializer) or
    parameters that each have `=` (default argument), decided by the
    tentative scanner (`comma_ends_saved_initializer`); otherwise the `<`
    is known to open template arguments and later commas inside it are
    kept. `template name <` is known up front.
  - `capture_default_arg` (at `=`), `capture_exception_spec` (at
    `noexcept`/`throw`, through the `)`), and
    `capture_function_prologue` (an optional `try`, a ctor-initializer and
    the body's `{`; clang's `ConsumeAndStoreFunctionPrologue`), after
    which the body is captured through its `}` (and `catch` handlers).
- **Replay** (`parser.rs`): `begin_replay(toks, end)` saves the
  position, current token, previous end and bracket depths, and makes
  `toks[0]` current. After the last saved token every read sees an
  `Eof` at `end`, so no rule runs past the saved range and
  "expected ..." diagnostics point at the token that ended it.
  `end_replay()` restores the interrupted stream and returns whether
  every saved token was consumed; replays nest (a nested class's items
  replay inside the outer class's). `inject_semi` uses the same stack to
  insert the `;` clang assumes after a class with a missing `;`.
- The token source's window (§2.1) is unaffected: saved tokens are owned
  by the parser, not the window.
- Pragma events (§2.3) keyed to tokens inside a saved range fire when the
  range is captured, not when it is replayed.

### 2.5 The template-argument scan memo (WP-086)
At each `<` after a template name the parser asks whether the
template-argument-list closes before an operator could
(`skip_template_args(i)`, [temp.names]/4). The scan classifies every name
in the list and recurses into nested lists, so without a memo each of `N`
nesting levels re-scans everything inside it: `W<W<...<char>...>>` was
quadratic in `N` (2,000 levels took 1.4 s in release).
- A named cast keyword (`static_cast`, `dynamic_cast`, `reinterpret_cast`,
  `const_cast`) followed by `<` opens a nested list, so
  `C<static_cast<T>(e)>` closes at the last `>` (WP-130).
- In `X<args>::name` the arguments are not converted: `name` is
  classified by the primary template's pattern, and, when the pattern
  lacks it, by the explicit and then the partial specializations
  (`Sema::classify_template_member`, WP-130). With `X` an alias template
  the alias's pattern is followed (through further alias templates) to
  the class template it specializes, whose member is classified the same
  way (`Sema::classify_alias_template_member`, WP-278), so
  `S<A<int>::type>` is a type-id; an alias template whose pattern names
  no class template (`decltype(...)`) is parsed ahead like an
  unresolved specialization scope (`annotate_template_scope`)
  (`tests/conformance/temp.alias/alias-template-id-qualified-type-as-template-argument.cpp`).
- A typedef qualifier `T::name` whose `T` names a class template
  specialization that is not instantiated yet (and is not an explicit
  specialization) is classified the same way, by the specialization's
  template: the lookahead cannot instantiate, and an undefined class has
  no members to find (`typedef X<int> T; W<T::rebind<float>, int>`,
  WP-276).
- `skip_template_args(i)` memoizes its result by token index `i` in
  `Parser::targs_memo`.
- A result is valid while `Sema::lookup_stamp()` is unchanged. The stamp
  hashes what name classification reads: the declaration count, the
  lookup tables' generation (bumped on every insertion), the semantic
  context, the scope stack (kind, entity, local and using-directive
  counts) and the prototype-parameter scopes. A different stamp empties
  the memo before the lookup.
- The token stream changing also empties it: `begin_replay`,
  `end_replay`, `inject_semi` (and the pop of an inject frame in `bump`)
  re-number the indices, and `split_greater` rewrites the current token.
- Parsing `W<...>` nested `N` deep is linear in `N`: the outermost scan
  fills the memo for every nested `<`, and the nested parses hit it.
  `bench/frontend/nested-template-ids.cpp` (128 aliases of a depth-2,000
  template-id) tracks it.

## 3. Parser core (`parser.rs`)
- **Token access:**
  - `tok`, `peek`, `bump`, `at(p)`, `at_kw(k)`, `eat`, `eat_kw`
  - `ident()` returns a non-keyword identifier.
  - Keywords are identifiers that `keywords.rs` maps to `Kw` with one
    hash lookup. The table includes the GNU spellings (`__inline__`,
    `__attribute__`, ...).
  - C-only keywords that clang marks KEYNOCXX are not in the table
    (WP-183). `_Bool` is a plain identifier in C++, so `_Bool b;`
    gives "unknown type name '_Bool'", and `sizeof(_Bool)` gives "use
    of undeclared identifier '_Bool'", as in clang. `_Atomic`,
    `_Alignas` and `_Alignof` stay, since clang accepts them in C++.
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
  - `condition_has_init_statement` (stmt.rs) finds whether a `;`
    precedes the `)` of an `if`/`switch` condition, skipping nested
    brackets and braces (clang's `SkipUntil(r_paren, semi)`), so an
    init-statement may hold braces (`if (int n{1}; n)`).
  - `parse_nested_name` consumes `A::B::`.

## 4. Declarations (`decl.rs`)
- **`parse_decl_specifiers`** fills a `kiln_sema::DeclSpec`:
  - storage class, `typedef`, `inline`, `thread_local`, `mutable`
  - `constexpr`, `consteval`, `constinit`: at most one of them
    ([dcl.constexpr]/1; `cannot combine with previous 'X' declaration
    specifier`)
  - function specifiers `virtual` and `explicit` / `explicit(
    constant-expression )` ([dcl.fct.spec]; the condition goes to
    `DeclSpec::explicit_expr`), and `friend`
  - type specifiers, cv-qualifiers, sign/width keywords
  - a type name (`NameClass::Type`) or tag specifier
  - attributes

  With `friend`, a storage class or `thread_local` (clang's
  `DeclSpec::Finish`, [class.friend]/6) is `'X' is invalid in friend
  declarations` (`parse_friend_decl_spec`) at the specifier (at
  `thread_local` with the combined name when both appear: `'static
  thread_local'`), and both are dropped, so the friend declaration goes
  on (`reject_friend_storage`, WP-293;
  `class.friend/friend-storage-class.cpp`). The parser records where each
  cv-qualifier was first written (`DeclSpec::cv_locs`); a friend type
  declaration (no declarator: `friend const T;`, or an element of a
  friend-type-specifier-list) reports each of `const`, `volatile`,
  `restrict`, `_Atomic` as `'const' is invalid in friend declarations` at
  the qualifier, in that order, and is still declared
  (`reject_friend_type_quals`, [class.friend]/3, WP-367;
  `class.friend/friend-type-cv.cpp`). A type-specifier-seq takes
  no storage class ([dcl.type.general]/1; clang
  `ParseSpecifierQualifierList`): in `DeclaratorContext::TypeName` (every
  type-id — casts, `sizeof`, `alignof`, `typeid`, template arguments,
  alias-declarations, trailing return types —, a new-type-id, also
  parenthesized, a conversion-type-id and a functional cast's type),
  a condition declaration (`parse_condition`, WP-293) and an
  exception-declaration (`parse_handler`) a storage class or
  `thread_local` is `type name does not allow storage class to be
  specified` at the specifier (else at `thread_local`) and is dropped
  (`reject_type_name_storage`, WP-327). spec/sema.md [dcl.stc] lists
  every context that rejects a storage class.

  Sema checks the combination when it converts the DeclSpec to a type
  (`[dcl.type.simple]`, `type_from_declspec_at`).
- **`parse_declarator`** fills a `Declarator`:
  - the name, its qualifier and its location
  - `chunks`, innermost first: pointer/reference with cv, array with
    bound, function with parameters, variadic flag and exception
    specification, and a member pointer `nested-name-specifier * cv`
    (`Chunk::MemberPointer`, [dcl.mptr]). `at_member_pointer` scans
    `::`? `name` (`<args>`)? `::` ... `*` without consuming; the
    nested-name-specifier is parsed with entering-context off, and when
    it fails or does not end at the `*` (`int nope::*p`) the tokens up
    to the `*` are skipped, the declarator is marked invalid and the
    chunk carries `qual: None`. The same form is accepted in abstract
    declarators and in new-declarators (`new int C::*`).
  - the bit-width, attributes, and `init_style`
  - `parens`: every pair of grouping parentheses, in the order they
    close, as (number of chunks inside, `(`, `)`) (clang's
    `DeclaratorChunk::Paren`; WP-191), for `ParenType` sugar and the
    redundant-parentheses warning; `after_comma`: a `,` precedes the
    declarator in its declaration (clang `!isFirstDeclarator()`)
  - a special declarator-id (`Declarator::special`, `SpecialName`):
    a constructor (the class's own name in member context, or `C::C`
    out of line, followed by `(`), `~C` (`expected a class name after
    '~' to name a destructor`), `operator @` (every overloadable
    operator, `new[]`/`delete[]`, `()`, `[]`, `co_await`), and
    `operator conversion-type-id` (its declarator takes only `*`, `&`,
    `&&` and cv-qualifiers, [class.conv.fct]/1). `operator""` is
    "literal operators is not supported yet". Special declarator-ids are
    recognised at namespace and class scope only; a decl-specifier-seq
    with no type is allowed before a constructor, destructor or
    conversion function.
  - an attribute-specifier-seq right after the declarator-id
    (`declarator-id attribute-specifier-seq(opt)`, [dcl.decl.general]/4),
    for every kind of declarator-id: an identifier or qualified-id
    (`f [[deprecated]] (int)`, a member template `g [[nodiscard]] (T)`),
    an operator-function-id (`operator() [[nodiscard]] (int)`,
    `operator new [[a]] (size_t)`) and `~C [[a]] ()`. It lands in
    `Declarator::attrs`, so it appertains to the declared entity. After a
    conversion-type-id the attributes are parsed with the type (they
    appertain to the type, as in clang).
  - on a function chunk: cv-qualifiers (a duplicate warns
    `duplicate 'const' declaration specifier`), a ref-qualifier, and an
    explicit object parameter (`this T self`, [dcl.fct]/6) in
    `Declarator::explicit_object`; in the tentative parse `this` followed
    by a decl-specifier starts a parameter-declaration-clause, so
    `void f(this int);` at namespace scope is a function declarator
  - in member context: virt-specifiers `override`/`final` (a repeat is
    `class member already marked 'X'`; `final` in a friend declaration
    is `'final' is invalid in friend declarations`) and a
    pure-specifier `= 0` (other initializers of a function are
    `initializer on function does not look like a pure-specifier`)
  - outside member context, `final`/`override` after a function
    declarator (and its requires-clause) are each `'X' specifier is not
    allowed outside a class definition` (`parse_virt_specifier_outside_class`)
    and skipped, so the declaration or definition goes on (WP-288)

  The parser peeks at `=`, `(` or `{` to set `init_style` *before* the
  declaration exists, because Sema needs it to create the `VarDecl`.
  Sema builds the type from the last chunk outwards ([dcl.meaning]).
  - A `(` after the declarator-id is a parameter list or a direct
    initializer by `is_cxx_function_declarator` (only where a direct
    initializer may follow; in member context it always starts a
    parameter list, since a member has no direct-initializer). It is a
    parameter list only if its contents parse as a
    parameter-declaration-clause: `T f(T a)`, `T f(T(a))`, `T f(T(*a))`
    and `T f(T())` declare functions, while `T x((a))`, `T x((T()))`,
    `T x((T(a)))` and `T x(T() + T())` declare variables (a
    parameter-declaration cannot begin with `(`, and no declarator is
    followed by `+`); the initializer's expression is then parsed as
    below (an ambiguous parenthesized type-id); when the choice was
    ambiguous the function chunk records it and Sema warns
    `-Wvexing-parse` (`parentheses were disambiguated as a function
    declaration` / `empty parentheses interpreted as a function
    declaration`, with clang's fix-it notes).
  - Parameters: `= initializer-clause` default arguments go to
    `act_on_param_default_init` (a default argument starting with `({`
    is `err_stmt_expr_in_default_arg`, skipped to `,`/`)`). In a member
    declarator whose function chunk applies to the name directly,
    default arguments and a parenthesized *noexcept-specifier* are
    saved instead (§2.4): the parameter gets `DefaultArg::Unparsed(=,
    arg)` (the `=` and the argument's first token), the declarator
    `late_noexcept`, and the chunk `ExcSpec::None` until replay. A friend
    function or friend template declared in the class has its
    *noexcept-specifier* saved too (WP-269); a qualified friend
    (`friend void N::f() noexcept(...)`) does not. The
    parameter-declaration-clause brackets `act_on_start_prototype` /
    `act_on_prototype_param` / `act_on_end_prototype` (closed after the
    trailing return type): each parameter is declared at its point of
    declaration and is in scope in the rest of the declarator
    (sema.md §4.3); a member's exception specification and trailing
    return type are bracketed by `act_on_start_this_scope` /
    `act_on_end_this_scope`.
    An unnamed parameter (and an abstract `for`-range or condition
    declarator) is located where its name would be (`set_abstract_loc`,
    clang `ParseDirectDeclarator`): the first array bound, else the token
    after the declarator.
  - `-> type-id` after the exception specification is a trailing
    return type (`Chunk::Function::trailing`), required with a
    leading `auto` and diagnosed otherwise by Sema.
  - `decltype ( expression )` and `decltype(auto)` are type specifiers;
    the operand is unevaluated.
  - GNU `typeof` (`parse_typeof_specifier`; spec/sema.md §5.8):
    `__typeof`/`__typeof__` (and `typeof`, a keyword only under
    `-std=gnu++NN`, `Keywords::new(_, gnu)`) and
    `__typeof_unqual`/`__typeof_unqual__` (no `typeof_unqual` in C++)
    followed by the operand of `sizeof` (`parse_trait_operand`, clang's
    `ParseExprAfterUnaryExprOrTypeTrait`): `( type-id )`, or an
    unevaluated unary-expression, a parenthesized one taking its postfix
    suffix (`__typeof(x)(y)` is the type of the call `x(y)`). As clang,
    `__typeof_unqual` does not begin a type-id in the disambiguator
    (`tentative.rs`) or a functional cast, so a template argument or a
    C-style cast cannot start with it ("expected expression").
    A concept name followed by `auto` or `decltype ( auto` begins a
    declaration in the statement disambiguator (`tentative.rs`), so
    `std::same_as<X> decltype(auto) r = e;` in a block is a constrained
    placeholder declaration (WP-169).
  - `_Atomic ( type-id )` (a clang extension in C++, C11 6.7.2.4) is a
    type specifier, built by `Sema::act_on_atomic_type` (array, function,
    reference, atomic and qualified operands are
    `sema_atomic_specifier_bad_type`). The bare `_Atomic` qualifier
    (WP-154) is parsed with the cv-qualifiers (`tentative.rs` treats it as
    a decl-specifier when not followed by `(`) and applied as clang's
    `BuildQualifiedType`: `_Atomic T` is `_Atomic(T)`, with the same
    operand checks.
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
  `parse_initializer`/`act_on_init` or `act_on_uninit`.
  - The first function declarator followed by `{`, `:`, `try`,
    `= default` or `= delete` is a definition
    (`parse_function_definition_after_declarator`): the body goes
    through `act_on_start_function_def`, `act_on_mem_initializers` (for
    a constructor, §4.1), the compound statement and
    `act_on_finish_function_def`; `= default;` / `= delete;` /
    `= delete("reason");` (P2573) go to `act_on_defaulted` /
    `act_on_deleted`. When Sema made no function the body is skipped.
    Where a definition is not allowed (block scope), `{` is
    `function definition is not allowed here`.
  - `= delete` / `= default` on a later declarator or a non-function:
    `'= delete' is a function definition and must occur in a
    standalone declaration`, `only functions can have deleted
    definitions`, `only special member functions and comparison
    operators may be defaulted`.
  - A declarator whose special declarator-id was malformed
    (`operator;`) takes the error type, so no `a type specifier is
    required` follows.
  - At namespace scope a declaration with no decl-specifiers whose
    first token is a name followed by `;`, `=`, `,`, `[`, `(` or `{`
    (`at_implicit_int_declarator`; `y;`, `y();`) is a declarator, not
    `expected unqualified-id`: Sema reports `a type specifier is
    required for all declarations` at the name and declares it as an
    invalid `int` (clang's implicit-int recovery, WP-371).
  - Group stickiness (clang's `Declarator::InvalidType` surviving
    `Declarator::clear`): after a declarator without a type, or one whose
    type named an unexpanded pack (`Sema::take_declarator_type_invalid`,
    spec/sema-templates.md), the later declarators of the group are
    parsed but not declared and take no initializer.
  - A declaration with no declarator (`struct S;`,
    `union { int a; };`) goes to `act_on_decl_spec_only`.
  - That call returns the implicit object of an anonymous union, if
    any, which then joins the `DeclStmt`.
- **GNU `__extension__`** before a `using` or `template` declaration at
  namespace, class or block scope is dropped (`skip_extension_before_using`;
  clang skips it in `ParseExternalDeclaration`): libc++ `__extension__
  using`, libstdc++ `__extension__ template<>` (WP-237). At namespace
  scope only, it is also dropped before an asm-declaration (libstdc++
  `__extension__ asm(...)`, WP-271); in a class or block `asm` after it
  is an error, as clang.
  `tests/conformance/ext.gnu.extension/`. Elsewhere it is a
  decl-specifier or the unary operator.
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
    declaration, definition or friend, decided by the next token, as in
    [dcl.type.elab]. A named class followed by `final` and `{` or `:`
    is a definition; a repeated `final` is `class already marked
    'final'`. `friend class X;` (or `friend class X` followed by `,` or `...`
    in a friend-type-specifier-list) is `TagUse::Friend`; a friend that
    would define a class is `cannot define a type in a friend
    declaration` and is skipped. An unqualified `friend class T;` whose
    name is a template parameter is `declaration of 'T' shadows template
    parameter` ([temp.local]/6, `check_template_param_shadow` in
    `act_on_friend_tag`, WP-367) and declares a new class `T`, so
    `friend class Ts...;` then expands no pack
    (`temp.local/p6-friend-class-shadow.cpp`).
  - Then `act_on_tag_attrs` for the class-head or enum-head attributes.
  - Then the body:
    - `parse_record_body` (§4.1) holds the base clause, member
      declarations, bit-fields and nested records.
      `act_on_finish_record_definition(r, then_semi)` is told whether a
      `;` follows, which decides whether an unnamed class is anonymous.
    - After a class or enum definition the next token must be able to
      follow the type (clang's `isValidAfterTypeSpecifier`); otherwise
      `expected ';' after struct|class|union|enum` at the end of the
      `}` and a `;` is injected (§2.4), so the next declaration parses
      normally. An enum in a member declaration may be followed by `:`
      (a bit-field).
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
    written) and an unqualified-id (an identifier, an operator-function-id
    or a conversion-function-id, WP-149) with an optional `...`, passed to
    `Sema::act_on_using_declarator` one at a time as a `DeclName`.
    - `typename` declarators (outside a class) and destructor names are
      "not supported yet".
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
  The message is an unevaluated string literal when every token up to
  the `)` is a string literal without a ud-suffix (clang 22's
  lookahead); otherwise it is a constant-expression handed to Sema
  (P2741, [dcl.pre]/10), and before C++26 that is
  `parse_ext_static_assert_user_message` (`-Wc++26-extensions`).
- **Varargs ellipsis without a comma** ([dcl.fct]/3,
  [depr.ellipsis.comma], C++26 P3176): in C++26, `(int...)` or
  `(int n...)` — a `...` that follows a parameter-declaration without a
  comma and does not make that parameter a pack — warns
  `parse_deprecated_missing_comma_before_ellipsis`
  (`-Wdeprecated-missing-comma-variadic-parameter`, in `-Wdeprecated`)
  at the `...`. A lone `(...)`, `(int, ...)` and a function parameter
  pack are not deprecated. Before C++26, nothing.
  In every mode, when the parameter before that `...` has its own pack
  `...` (`(Ts......)`, `(Ts...a...)`) or a type naming an unexpanded
  pack, clang's `-Wambiguous-ellipsis` follows (WP-364, clang
  `warn_misplaced_ellipsis_vararg`, after the deprecation in C++26):
  `'...' in this location creates a C-style varargs function` at the
  `...` (`, not a function parameter pack` appended when the parameter
  has no `...` of its own), a note `preceding '...' declares a function
  parameter pack` at the pack's `...` (or `place '...' immediately
  before declared identifier` / `here` at the name or the declarator),
  and `insert ',' before '...' to silence this warning` at the `...`.
  Tests: `dcl.fct/p3176-ambiguous-ellipsis.cpp` (C++23),
  `dcl.fct/p3176-variadic-comma.cpp` (C++26, clang >= 22).
- **Initializers** (`parse_initializer`, crate-visible):
  - `= expr`, `= { … }`, `( expr )` and `{ … }`
  - designated initializers `.x = …` (C++20)
  - The result is a `kiln_sema::Initializer` tree.

### 4.1 Classes (`class.rs`)
- **Class body** (`parse_record_body`, clang's `ParseCXXMemberSpecification`):
  `act_on_start_record_definition`, then `act_on_class_final` for a
  class-virt-specifier, then `act_on_base_specifiers` (always, with an
  empty slice when there is no base clause), the members, `}` and
  `act_on_finish_record_definition`. A missing `{` after a base clause
  is `expected '{' after base class list`; the class is still finished.
- **Base clause** ([class.derived]): `[attrs] [virtual] [access]
  [virtual] class-or-decltype [...]`; a second `virtual` is
  `duplicate 'virtual' in base specifier`. Each base becomes a
  `BaseSpecInfo`. A class-or-decltype is a decltype-specifier, a
  simple-template-id or a (possibly qualified) class-name; the qualifier
  may begin with `decltype(e)::` (WP-119).
- **Member declarations** (`parse_member_declaration`, clang's
  `ParseCXXClassMemberDeclaration`):
  - leaders: `;`, `public:`/`protected:`/`private:` (a missing `:` is
    `expected ':'`; `act_on_access_spec`), `static_assert`, `using`,
    `template` (unsupported).
  - `decl-specifier-seq ;`: `act_on_friend(ds, base, None, loc)` for
    `friend class X;`, else `act_on_decl_spec_only`.
  - No decl-specifiers and no special declarator: a name followed by
    `;`, `=`, `,`, `[`, `(` or `{` (`at_implicit_int_declarator`) is
    parsed as a member-declarator and declared as an invalid `int`
    member (data member or, for `y();`, member function, so `S::y`
    resolves and its uses are silent) after `a type specifier is
    required for all declarations` at the name, once per declarator
    (clang's implicit-int recovery, `err_missing_type_specifier`;
    WP-371). Otherwise (`member_without_type`, WP-367) `()` is
    `expected member name or ';' after declaration specifiers` at the
    `)` (clang's parenthesized declarator), anything else at the token,
    and the declaration is skipped to its `;`.
    `class.mem/member-without-type.cpp`,
    `dcl.type/missing-type-specifier-implicit-int.cpp`.
  - A friend-type-specifier-list ([class.friend]/3, P2893, WP-359): with
    `friend` and the next token `...` or `,`, `parse_friend_type_list`
    takes each friend-type-specifier, an optional `...`, and calls
    `act_on_friend_type(ds, base, ellipsis, friend_loc)`. The later
    specifiers are parsed by `parse_decl_specifiers_from(Member,
    Some(friend_loc))`, so `friend` applies to them and an elaborated
    `struct A` before `,` or `...` is `TagUse::Friend`. A specifier with no
    type is `a type specifier is required for all declarations` (skipped
    to `;`); a missing `;` is `expected ';' after friend declaration`,
    with nothing skipped: the rest is parsed as the next member
    declaration (clang's `ExpectAndConsume`; `friend A, B int y;` goes on
    with `y;`, `class.friend/p2893-recovery.cpp`).
    Before C++26 the list warns `parse_ext_variadic_friend` ("variadic
    'friend' declarations are a C++2c extension", `-Wc++26-extensions`)
    at its first `...` or `,`.
  - member-declarators: each goes to `act_on_declarator` with
    `DeclaratorContext::Member` (or `act_on_friend` with the declarator
    for a friend). A bit-field `: width` may carry a default member
    initializer (C++20); an unnamed one may not (`anonymous bit-field
    cannot have a default member initializer`). A missing `;` is
    `expected ';' at end of declaration list`; an unparseable
    declaration is skipped as clang's `SkipMalformedDecl` does.
  - A function definition's body, ctor-initializer included, is saved
    (§2.4) when Sema made the function; `= default`/`= delete` and
    `= 0` are handled in place. A static data member's initializer is
    parsed in place (it is not a complete-class context).
  - A default member initializer (`= x` or `{x}`) is saved; the field
    is created first, with its `InitStyle`.
- **Delayed parsing.** The parser keeps a stack of `LateClass { record,
  items }`, one per class being defined. Items are, in declaration
  order: `Method` (saved default arguments and noexcept of a member
  function), `Nsdmi`, `Body`, and `Nested(LateClass)` for a nested class
  with items. After the outermost class's `}` and
  `act_on_finish_record_definition`, three phases run over the class
  tree (clang's `ParseLexedMethodDeclarations`,
  `ParseLexedMemberInitializers`, `ParseLexedMethodDefs`). Each class
  that has items of the phase is re-entered with
  `act_on_start_delayed_class` / `act_on_finish_delayed_class`, the
  nested ones inside their parent's, in declaration order:
  1. **Method:** `act_on_start_delayed_method(f)`, then each default
     argument (`act_on_param_default_arg(p, e)`; leftover tokens are
     `unexpected end of default argument expression`; a braced default
     argument is parsed as an *initializer-list* and goes to
     `act_on_param_default_list(p, list)`, WP-136) and the *noexcept-specifier*
     (`act_on_exception_spec(f, spec)`; leftovers are `unexpected end of
     exception specification`), then `act_on_finish_delayed_method(f)`.
  2. **Nsdmi:** `act_on_start_field_init(field)`, the initializer, then
     `act_on_field_init(field, init)` if it parsed (not for a field Sema
     marked invalid).
  3. **Body:** the function definition as at namespace scope (§4).
  Then `act_on_finish_class_contexts(r)`. A nested class finishes its
  definition at its own `}` but replays with the outermost class. The
  exact call sequences are the `#[cfg(test)]` trace tests in `class.rs`
  (§10).
- **Ctor-initializers** (`parse_mem_initializer`, [class.base.init]):
  `mem-initializer-id ( expression-list ) | braced-init-list [...]`,
  separated by `,` (a missing one between two initializers is `missing
  ',' between base or member initializers`). The id is a `MemInitId`:
  an identifier, or a type for a qualified class name or a
  decltype-specifier (`decltype(e)(args)`, class-or-decltype; a dependent
  one is resolved at instantiation, and a type that is not a class is
  `constructor initializer ... does not name a class` in sema;
  `decltype(auto)` is `'decltype(auto)' not allowed here` and counts as
  a failed initializer, `QualType::ERROR`); qualified non-types are
  unsupported. All initializers go to
  one `act_on_mem_initializers(ctor, inits, colon)`; a constructor
  without a ctor-initializer gets an empty slice at its `{`. A
  ctor-initializer on a non-constructor is `only constructors take base
  initializers`.
- **`this`** goes to `act_on_this(loc)`.

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
  - When that type-id was *ambiguous* (the second flag: the abstract
    declarator could also be an expression, `(T())`, `(T(*)())`), the
    tokens after the matching `)` decide ([dcl.ambig.res]/2,
    [expr.cast]; clang's `ParseCXXAmbiguousParenExpression`,
    `ambiguous_paren_is_cast`): a `{` makes it a compound literal, `(`
    `)` makes it an expression, and any other token is a cast exactly
    when it can begin a cast-expression (`starts_cast_expr`, the tokens
    clang's `ParseCastExpression` accepts without `NotCastExpr`:
    literals, identifiers and the keywords that start a primary or
    unary expression or a functional cast, `(`, `[`, `::`, `^^`, `[:`,
    and the unary operators `++ -- & && * + - ! ~`). So `(T())x` and
    `(T())*x` are casts, while `(T());`, `(T())/x`, `(T())()` and the
    direct-initializer `T x((T()))` (libc++ `count_new.h`'s
    `MemCounter counter((MemCounter::MemCounterCtorArg_()))`) are
    parenthesized expressions. Unlike clang, kiln decides by looking
    ahead without parsing the operand first; the only difference is
    that the cast's type is built before its operand.
  - `throw [assignment-expression]` (the operand is optional before
    `)`, `]`, `}`, `;`, `,` and `:`) and `typeid ( type-id |
    expression )`, disambiguated with `is_cxx_type_id`; the operand of
    a type-id-shaped `typeid` is unevaluated.
  - Then postfix operators (`parse_postfix_suffix`): calls, subscripts,
    `.`, `->`, `++`, `--`. A call argument may be a braced-init-list
    (`act_on_braced_arg`, converted once the parameter type is known).
    A member destructor name may be a template-id, `p->~C<T>()`
    ([expr.prim.id.dtor]/1): the template-id is parsed as a type and
    passed to `Sema::rebuild_member_destructor` (WP-149). A qualified
    one, `x.N::~T()`, is parsed with the nested-name-specifier as the
    qualifier; when the object is not of class type or is dependent
    (`Sema::may_be_pseudo_destructor`) and the designator is
    `[nns] type-name :: ~ type-name`, the last `type-name ::` is not a
    nested-name-specifier but the pseudo-destructor's first type-name
    (clang's `MayBePseudoDestructor`; `parse_pseudo_destructor`). Both
    go to `Sema::act_on_member_destructor_q` (WP-223).
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
    - `__builtin_va_arg ( assignment-expression , type-id )` (WP-171) goes
      to `act_on_va_arg` with the type's begin location and the `)`.
      Errors as clang: `expected '(' after '__builtin_va_arg'`,
      `expected ','`, a missing `)`.
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

- **Coroutine operators (WP-351):**
  - `co_await cast-expression` ([expr.await]/1) is a unary operator
    parsed in `parse_cast_expr` (clang's `ParseCastExpression`,
    `kw_co_await`); it goes to `act_on_coawait(operand, kw_loc)`.
  - A yield-expression ([expr.yield]/1) is an alternative of
    assignment-expression, tried before the conditional-expression
    (like `throw`): `co_yield assignment-expression` or
    `co_yield braced-init-list` (the list goes through
    `act_on_braced_arg`), then `act_on_coyield(operand, kw_loc)`.
  - `operator co_await` is an overloadable operator name (§4.1).
  - Where the keyword may appear (a function body, not a handler, not
    unevaluated) is Sema's call (spec/sema.md §"Coroutines").

### 5.1 New- and delete-expressions (`new_delete.rs`, WP-061a)
clang's `ParseCXXNewExpression`, `ParseDirectNewDeclarator` and
`ParseCXXDeleteExpression`; `parse_cast_expr` dispatches at `new`,
`delete`, and `::` followed by either.
- **Placement or type-id.** After `new (`, `paren_is_type_id` decides
  without a tentative parse (clang's `isTypeIdInParens`; TODO(M3) for
  the shapes needing one): the tokens must start a type-id; then
  simple-type keywords and type names are skipped, and the next token
  decides. `{` means an expression (`new (int{1}) T` is a placement); a
  `(` is a type-id only when followed by `* & && ^ ) [ ( ...`, a type-id
  start or a decl-specifier keyword, so `new (short(n), 'x') int` is a
  placement and `new (int (*)[2])` a type-id. A `(` followed by a
  member-pointer declarator (`new (void (T::*)())`, `member_pointer_at`;
  WP-216) is a type-id too. `decltype`, `typeof`, `__typeof_unqual`,
  `typename` and a class-key are always a type-id, and so is a
  decl-specifier keyword that is no type-specifier (`new (static int)`,
  clang `isCXXDeclarationSpecifier`; its storage class is then rejected,
  §4, WP-327).
- **new-type-id.** The decl-specifiers and an abstract new-declarator
  (`*`, `&`-free ptr-operators, then `[expr]` bounds); the first bound
  is an expression, the later ones constant expressions (checked by
  Sema). A parenthesized `( type-id )` may follow a placement:
  `new (p) (int)(5)`.
- **new-initializer.** `( expression-list(opt) )` or a braced-init-list
  (`parse_init_list`). The result goes to `Sema::act_on_cxx_new(global,
  start, placement, NewTypeId, NewInit, end)`.
- **delete.** `delete`, then `[` `]` for the array form (clang's lambda
  disambiguation of `delete []{...}` is M3 with lambdas), then a
  cast-expression; `Sema::act_on_cxx_delete`.
- **No postfix suffix.** A new-expression is a unary-expression: a
  following `[`, `(`, `++`, `--`, `.` or `->` reports `expression cannot be
  followed by a postfix '[' operator; add parentheses` and parses the
  suffix for recovery (except `[`/`(`/`++`/`--` at the start of a line,
  where the caller's `expected ';'` is better).

### 5.2 Reflection (`reflect.rs`, WP-267 spike, `-freflection`)
- The tokens `^^`, `[:` and `:]` exist only when `ParseOptions::reflection`
  is set and the standard is C++26 (spec/lex.md Phase 3). Without the
  flag, `^^x` is `^` `^x` and gets clang 19's errors
  (`tests/conformance/reflection/flag-off.cpp`).
- **`^^` operand** ([expr.reflect]). If the operand can be a type-id (the
  test is the trailing-return-type one, so `void (*)(int)` is a type), it
  is parsed as a type-id → `act_on_reflect_type`. Otherwise it is an
  optional nested-name-specifier and an identifier →
  `act_on_reflect_name`. A template-id operand is
  "unsupported: reflection of a template-id".
- **Splice brackets.** `typename [: e :]` in a type position, and `[: e :]`
  as a template argument ([temp.arg.general]/3; the spike always treats it
  as a type), parse `e` as a constant-expression and pass it to
  `act_on_splice_type`. `splice_end` finds the matching `:]` (nested
  splices balance), and the tentative-parse scanner and template-argument
  scan use it to skip a splice as one unit. An expression splice is
  "unsupported: expression splice".

## 6. Statements and attributes

### 6.1 Statements (`stmt.rs`)
- **Declaration or expression** ([stmt.ambig], [dcl.ambig.res]):
  `is_cxx_declaration_statement` (§2.1) decides, as clang's
  `isCXXDeclarationStatement` does. A `[` after a declarator is clang's
  `TryParseBracketDeclarator`: `[{` or a top-level `,` before the `]`
  means an expression; `[[` is a declarator suffix (the attributes are
  then `an attribute list cannot appear here` after a parenthesized
  declarator, `discard_prohibited_cxx11_attrs`). A condition is a
  declaration when the declarator is followed by `=`, `{`, `[[`, `asm`
  or `__attribute__`. `condition_has_init_statement` looks for a `;` at
  the top level of the parentheses, skipping balanced groups, so
  `if (int n{1}; n)` has an init-statement.
- **Ambiguous names** ([basic.lookup.general]/1; WP-197): before that
  decision, a statement that starts with an unqualified identifier
  (not followed by `::`) asks Sema's `diagnose_ambiguous_name(name,
  None, loc)`. When the lookup is ambiguous it reports `reference to
  'x' is ambiguous` with candidate notes and the statement is skipped
  to its `;` (clang's `ClassifyName` error in
  `ParseStatementOrDeclaration`). Where a type name was already
  classified (a decl-specifier's simple type, a qualified
  `typename`-less type name) the same call reports the ambiguity and
  recovery keeps the classified type, as clang's `getTypeName` does.

  Audit against clang 19 (WP-180). `T` is a class, `U` a template type
  parameter, `a` a variable, `I` a typedef; tests are under
  `tests/conformance/stmt.ambig/` and `tests/conformance/dcl.ambig.res/`.

  | Form | clang 19 | kiln |
  |---|---|---|
  | `T x(T(a)), y;` | `x` a function with parameter `a`, `-Wvexing-parse` | same |
  | `T(a)->f();` | expression (cast) | same |
  | `T(*p)(int);` | declaration of `p`, `T*` from `int` | same |
  | `T(e)[3];` | declaration of an array | same |
  | `T(e) = 1;` | declaration | same |
  | `T(e), b;` / `T(e), y = 2;` | declarations | same |
  | `T(a)++;`, `T(a, 5) << c;`, `T(a)->m = 7;` | expressions | same |
  | `int(a)++;` | expression; `assignment to cast is illegal, lvalue casts are not supported` | same (fixed in WP-180) |
  | `T(f) = {1};` | declaration | same |
  | `T(*g)(nullptr);` | declaration, type sugar `T (*)` | declaration, `T *` (no ParenType sugar) |
  | `T(n) [[maybe_unused]];` | declaration; `an attribute list cannot appear here` | same (fixed in WP-180) |
  | `T(a);`, `T(a)(b);`, `U(a);` with `a` declared | declaration; `-Wvexing-parse` redundant-parentheses warning | declaration, no warning |
  | `void g(T(a));` / `void g(T(I));` | parameter `a` / parameter `T (*)(I)` | same |
  | `sizeof(T(a))`, `sizeof(T(a).m)` | expressions | same |
  | `sizeof(T())`, `sizeof(T(*)())` | type-ids (the first a function type: error) | same |
  | `typeid(T(a))` / `typeid(T())`, `typeid(T(I))` | expression / type-ids | same |
  | `for (T(e); ;)` | declaration | same |
  | `if (T(e) = 1)` / `if (T(a).m)` | declaration / expression | same |
  | `if (T(n){1})`, `while (T(n){1})` | declaration with a braced initializer | same (fixed in WP-180) |
  | `if (int n{1}; n)`, `switch (int m{2}; m)` | init-statement | same (fixed in WP-180) |
  | `(T()){}` | `illegal initializer type 'T ()'` | same (fixed in WP-180) |
  | `U(e);`, `U(*p)(int);`, `U(f), g = 1;`, `U(h)[2];` | declarations | same |
  | `(U)a`, `(U(a))`, `U(a) + 1`, `U(a)->m` | expressions | same |
  | `typename U::type(e);` / `U::type(a);` | declaration / call | same |
  | `X<U>::m d, e;`, `X<U>::m *p;`, `(X<U>::m)0;` | expressions (a dependent template-id qualifier) | same (fixed in WP-291) |
  | `U::m const a = 0;`, `U::m b, c;` | declarations; `missing 'typename'` | same (fixed in WP-291) |
  | `void h(U::m);` | variable; `variable has incomplete type 'void'` | same (fixed in WP-291) |
- **Expression statements** ([stmt.expr]): a missing `;` is `expected ';'
  after expression` at the end of the expression, and nothing is skipped:
  the next statement starts at the current token (clang's
  `ExpectAndConsumeSemi`), so `X<U>::m d, e;` goes on to report `d` and `e`.
  A statement `Q::name ...` whose class template specialization
  qualifier `Q` is undefined or failed to instantiate (diagnosed when the
  qualifier was parsed ahead) is neither, whatever follows the name
  (`Q::name *g;`, `Q::name & r;`): it is skipped to `;` silently
  and declares nothing (clang's `ClassifyName` `NC_Error`,
  `at_failed_scope_declaration`).
  A statement `S::name ...` with `S` a non-dependent class or namespace
  (or `::`) that has no `name` (`unresolved_scope_name`, clang's
  `ANK_Unresolved` after a scope) is decided by the declarator
  (`is_simple_declaration`, WP-300): `S::name(e);`, `S::name *g;`,
  `S::type d;` are declarations whose decl-specifier reports `no type
  named 'name' in 'S'` (`in namespace 'N'`, `in the global namespace`;
  `parse_type_name_spec`), and `S::name = 1;` is an expression (`no
  member named`). An identifier after the name that is a non-type, or
  undeclared before `(`, makes it an expression (clang annotates it,
  and no declarator accepts the annotation): `S::pn y(e);`. The same
  decl-specifier rule applies at
  namespace scope (`N::gt gv;`) and in a parameter clause, where the
  name is ambiguous (`param_decl_specifier`: `void a1(S::pn);` declares
  a function)
  (`tests/conformance/stmt.ambig/qualified-unresolved-name.cpp`).
- **Delayed `no member named`** ([stmt.expr]/1, WP-308): an
  expression statement brackets its expression and its `;` with
  `Sema::begin_delayed_typos` / `end_delayed_typos`. In between, a
  qualified name lookup does not find (`S::nom`, `N::nom`;
  `act_on_id_expression_dn` through `Sema::emit_delayed_typo`) is held,
  and emitted after the `expected ';' after expression`, in source order,
  as clang's delayed typo correction reports it at the end of the
  full-expression. Elsewhere (a declaration's initializer, a condition, a
  `return`, a template instantiation) and for an unqualified name or a
  member access (`s.nomember`) it is reported where it is written.
  Scopes nest: an inner scope's held typos stay held while an outer
  one is open (`end_delayed_typos`), and each held typo records the
  recovery expression that stands for clang's `TypoExpr`
  (`Sema::correct_delayed_typos_in` finds them in a subexpression).
  The points where clang's parser corrects a subexpression's typos
  early (WP-318, WP-330; `ParsePostfixExpressionSuffix`,
  `ParseExpressionList`):
  - A call whose `)` is missing (`f(S::nom x);`, `S::nom(1;`,
    `g(1, S::nom x);`) reports the callee's and the arguments' typos at
    once, before what follows (`a = g(S::nom x) + ;` reports the typo,
    then "expected expression"); if there were any, the parser skips to
    the `)` (stopping at `;`) with no "expected ')'", and the
    expression is invalid (clang's `ExprError`; `Parser::expr_invalid`):
    an expression statement missing its `;` after it (`a = g(S::nom x)`
    then a newline) skips to the next `;` with no "expected ';' after
    expression", as clang's `ParseExprStatement`. In a
    parenthesized expression (`(S::nom x);`) "expected ')'" and its note
    come first.
  - A subscript's base is corrected at the `]` (`S::nom[1 2]` reports
    the typo before "expected ']'"). Its index list is a scope of its
    own: with the `]` present it ends normally; with the `]` missing
    the index is dropped and its typos are kept
    (`Sema::drop_delayed_typos`) until the end of the translation unit,
    after the pending instantiations and before the
    `ActOnEndOfTranslationUnit` checks (`act_on_end_of_translation_unit`),
    as clang, which never corrects a dropped `TypoExpr` before then;
    this holds in a declaration's initializer too (`int v = arr[S::nom
    x];`).
  - An expression list that fails to parse (a call's or a
    subscript's) reports the typos of what it parsed, then the
    callee's or base's, after the syntax error.
  - A lambda body and a statement expression suspend the enclosing
    scopes (`Sema::suspend_delayed_typos`): their statements and
    declarations end their own full-expressions, so `[] { int v =
    S::nom; }(), 1 +;` reports the typo before "expected expression".
  Tests: `tests/conformance/stmt.expr/delayed-no-member-after-missing-semi.cpp`,
  `tests/conformance/stmt.expr/delayed-no-member-nested-full-expression.cpp`,
  `tests/conformance/stmt.expr/delayed-typo-correction-points.cpp`
  (kiln_parse `delayed_no_member_after_missing_semi`,
  `delayed_no_member_nested_in_expression_statement`,
  `delayed_typo_correction_points`).
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
  - An init-statement may be an alias-declaration (`using T = int;`,
    [stmt.pre]/1, C++23 P2360) in `if`, `switch`, `for` and, since
    WP-356, range-based `for`; before C++23 it is
    `parse_ext_alias_in_init_statement` (`-Wc++23-extensions`) at
    `using`, once the declaration has parsed (clang
    `ParseAliasDeclarationInInitStatement`).
  - A structured binding declaration may be the condition
    ([stmt.pre]/4, C++26 P0963); Sema owns its rules (spec/sema.md §4.9).
  - A condition declaration is `decl-specifiers declarator = expr` or
    `{ … }` (`act_on_condition_var`). Without an initializer the error
    is `variable declaration in condition must have an initializer`.
- **Labels:**
  - `case`, `default`, `identifier:`
  - A label at the end of a block labels a null statement (C++23).
- **Jumps:** `goto`, `break`, `continue`, `return`.
- **`co_return`** ([stmt.return.coroutine]/1, WP-351):
  `co_return expr-or-braced-init-list? ;` (`parse_coreturn`, clang's
  `ParseReturnStatement` for `kw_co_return`) goes to
  `act_on_coreturn(operand, kw_loc)`. A failed operand skips to the
  enclosing `}` and eats a `;`; a missing `;` is
  `expected ';' after co_return statement`.
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
  - `alignas(type-id)` is recognised by `scan_name`; a trailing `...`
    makes the operand a pack expansion (`act_on_alignas_operand`,
    spec/sema.md §4.5).
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
  - before a declaration statement in a block, when none of them is a
    statement attribute, for the declaration (WP-203): `parse_statement`
    hands them on through `Parser::stmt_attrs` / `lead_decl_attrs` and
    `parse_decl_specifiers` takes them into the `DeclSpec` (clang's
    `ParseSimpleDeclaration` with `DeclSpecAttrs`), so `[[maybe_unused]]
    int x;` marks `x`.
  - after the declarator-id (`int x alignas(8);`, [dcl.decl.general]/4,
    [class.mem.general]/1), for the declaration; `alignas` there may be
    followed by a bit-width (`int b alignas(8) : 3`, then rejected by
    sema on the bit-field) (WP-312). After a member's bit-width or asm
    label, or a member function declarator with virt-specifiers, an
    attribute-specifier-seq is not allowed ([class.mem.general]/1):
    each run of `[[...]]`/`alignas` specifiers is parsed, dropped and
    reported once as `an attribute list cannot appear here` at its
    start (clang `DiagnoseAndSkipCXX11Attributes`); GNU attributes
    between them are kept (`parse_member_tail_attrs`; WP-324). The
    same holds after a member function declarator's GNU attributes
    (`void f() __attribute__((cold)) [[nodiscard]];`): only the
    `[[...]]`/`alignas` run right after the parameter clause is the
    function type's (WP-381). Outside a class, after a declarator's asm
    label only GNU attributes follow (clang `ParseDeclGroup`,
    `ParseAsmAttributesAfterDeclarator`): a C++11 run there ends the
    declarator, so at namespace scope a first function declarator gets
    `expected function body after function declarator` and the
    declaration is skipped through its `;`, and any other declarator
    `expected ';' after top level declarator` (`expected ';' at end of
    declaration` at block scope)
    (`tests/conformance/dcl.attr.grammar/attribute-list-after-gnu-attributes.cpp`)
  - between decl-specifiers (clang `ProhibitAttributes` in
    `ParseDeclarationSpecifiers`), at the end of the decl-specifier-seq
    ([dcl.spec.general]/1), after a ptr-operator ([dcl.ptr]/1), after
    an array declarator ([dcl.array]/1) or after a function declarator
    ([dcl.fct]/1, parsed after `parse_function_suffix`): attributes
    there appertain to a type. The parser hands each list to
    `Sema::act_on_type_position_attrs`, which reports and removes the
    declaration attributes (spec/sema.md §4.5, type-position
    attributes). The rest are attached to the declaration at the end of
    the decl-specifier-seq and after a function declarator, and dropped
    after a ptr-operator or array declarator
    (`skip_type_position_attrs`; WP-312, WP-324)
  - leading, before a declaration whose grammar has no
    attribute-specifier-seq ([dcl.pre]/5, clang `ProhibitAttributes`):
    `an attribute list cannot appear here` at the list's start, and the
    list is dropped. At namespace scope (file scope or inside a
    namespace) this covers a namespace-definition or
    namespace-alias-definition, static_assert-declaration,
    template-declaration, explicit instantiation or specialization,
    linkage-specification, asm-declaration and export-declaration
    (`discard_attrs_before_unattributed_decl`); in a block, a
    static_assert-declaration and namespace-alias-definition
    (`parse_statement_after_attrs`, `prohibit_attrs_at`); in a
    member-specification, a `friend` declaration that is not a
    definition (`prohibit_friend_lead_attrs`). The list's start is
    recorded as `DeclSpec::lead_cxx11` (WP-324).
  - before `using` ([namespace.udir]/1, [namespace.udecl]/1,
    [enum.udecl]/1, [dcl.typedef]/2; `parse_using_leading`, in every
    scope): before a using-directive the attributes are the
    directive's (subject-checked by sema); before an alias-declaration
    `an attribute list cannot appear here` and they are dropped (they
    belong after the alias name); before a using-declaration or
    using-enum-declaration each `[[...]]`/`alignas` gets clang's
    `ext_cxx11_attr_placement` warning `ISO C++ does not allow an
    attribute list to appear here` at the attribute (the scope token of
    a scoped one), and a using-declaration's are then checked on each
    declared using-declaration.
  - before a declaration that declares only a tag (`[[x]] struct S
    { };`, `alignas(4) struct S;`, [dcl.pre]/5: the attributes would
    appertain to the declared entities, and there are none): at file
    or namespace scope, `misplaced attributes; expected attributes
    here` at the end of the class-key (`err_attributes_misplaced`);
    `[[x]] int;` gets `an attribute list cannot appear here`
    (file or block scope). Either way the leading C++11 attributes are
    dropped (`prohibit_free_standing_lead_attrs`, `drop_lead_cxx11`);
    GNU attributes stay. In a member-specification no error; sema's
    place-it warning (spec/sema.md §4.5) covers both.
  - before a statement that is not a declaration (`[[nodiscard]]
    return;`, [stmt.pre]/1): the non-statement attributes are reported
    by sema `act_on_stmt_decl_attrs` (`'X' attribute cannot be applied
    to a statement`; on a block-scope asm-declaration clang's `'X'
    attribute ignored` warning). `alignas` may also start such a list.
  - an attribute-declaration (`[[x]];`, `alignas(4);`, [dcl.pre]/1):
    at namespace scope its attributes are subject-checked against
    nothing (sema `act_on_decl_attrs` on the enclosing context); in a
    member-specification it is `declaration does not declare anything`.
  - after the class-name of a class-head (`struct S [[x]] { };`,
    `struct S alignas(4) final : B { };`, `struct S [[x]];`,
    `template<> struct A<int> [[x]] { };`, [class.pre]/1): `an
    attribute list cannot appear here`, and the attributes are moved
    onto the class (`parse_class_head_attrs`); after `final` (`struct S
    final [[x]] { };`) the same error and they are dropped
    (`parse_class_virt_specifiers`). A reference (`struct S [[x]] *p;`)
    keeps them as type-position attributes.
  - before an enumerator (`enum E { [[x]] e };`, [dcl.enum]/1: they
    follow its name): `expected identifier`, then parsing resumes at
    the next `,` (clang `ParseEnumBody`).
- **`[[assume(e)]]` (WP-160):** `parse_assume_arg` parses a
  conditional-expression. If the next token is not `)`, it reparses the
  full expression, reports `use of this expression in an 'assume'
  attribute requires parentheses` at its expression location, and drops
  the attribute. A statement whose leading `[[...]]` holds an `assume`
  goes through `act_on_attributed_stmt` after the sub-statement is parsed,
  passing the language standard (sema has none of its own). So does one
  holding `likely`, `unlikely` or `fallthrough` (WP-182; spec/sema.md).
- **Identifier argument (WP-230):** for `enum_extensibility` (clang
  `attributeHasIdentifierArg`) a leading identifier is passed as
  `AttrArg::Ident(name, loc)`; any further arguments, or a first one that
  is not an identifier, parse as expressions. `flag_enum` and
  `enum_extensibility` parse their arguments (so Sema can count them);
  other attributes outside `takes_expr_args` skip theirs.
- **No argument list (WP-182):** a standard attribute that takes no
  arguments (`carries_dependency`, `fallthrough`, `noreturn`,
  `no_unique_address`, `likely`, `unlikely`, `maybe_unused`), written
  unscoped in `[[...]]` with `(...)`, even empty, reports `attribute 'x'
  cannot have an argument list` at the `(`; its tokens are skipped and
  the attribute is dropped.
- **Attribute-list recovery (WP-182):** after an attribute in `[[...]]`,
  anything but `,` or `]` reports `expected ','` after the previous token
  (`[[assume x]]`, `[[assume(x)(x)]]`), then skips to the `]` (stopping
  at `;`) without consuming it, as clang's
  `ParseCXX11AttributeSpecifierInternal` does; there is no
  `expected ']'` / `to match this '['` pair. The argument-count error of
  an `assume` without `(...)` still comes from Sema.

## 7. Driver (`kiln++ -fsyntax-only [-fdump-ast]`)
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
- **Classes** (parsed by WP-065; their Sema is WP-066b):
  - qualified non-type mem-initializer-ids, braced
    default arguments
  - literal operators (`operator""`)
  - pragma events inside saved tokens (they fire at capture)
  - diagnostics clang gives that the parser skips: a decl-specifier
    after a virt-specifier, the missing-`{` fix-it after a base clause,
    a typedef function definition, virt-specifiers on a non-function
- **Templates:** declarations, template-ids, `typename`, `template`,
  packs, concepts, requires-clauses. Design: spec/sema-templates.md §2.
- **Other expressions:**
  - lambdas; `new`/`delete` of class types (§5.1)
  - `dynamic_cast` semantics, polymorphic `typeid`
  - class and array compound literals, compound literals at namespace
    scope, the GNU address-of-label operator
  - class `T()`, `T{…}` and `T(a, b)` temporaries
  - fold expressions
  - braced-init-list subscripts and functional-cast operands
  - pointer-to-member operators on classes
  - qualified member access `e.N::m` (reported "qualified member
    access is not supported yet": `act_on_member` takes no
    nested-name-specifier)
  - reflection beyond the `-freflection` spike (§5.2): expression,
    dependent and template splices, `^^` of a template-id
- **Statements:** range-based `for`, contracts.

**Landed (WP-100).** `try` compound-statement handler-seq ([except.pre]/1)
in `stmt.rs`: a handler is `catch ( exception-declaration )
compound-statement`, the declaration `...` or a type-specifier-seq with
an optional (possibly abstract) declarator, sent to
`act_on_exception_decl`; the handlers to `act_on_catch` and the whole to
`act_on_try`. A `try` without a handler is `expected catch`. A
function-try-block (`class.rs` captured prologue and the free-function
path) parses `try`, then the ctor-initializer, then the body and its
handlers; the body statement given to `act_on_finish_function_def` is the
`CXXTryStmt` (as clang's AST: no enclosing `CompoundStmt`).

**Landed (WP-119).** `decltype ( expression ) ::` as the first component
of a nested-name-specifier ([expr.prim.id.qual]/1), in every place a
nested-name-specifier is parsed: decl-specifiers, `typename`,
id-expressions, template-ids (`decltype(e)::template M<T>`),
base-specifiers, member-pointer declarators (`int decltype(e)::*p`),
using-declarations, template arguments, conditions and statements. The
parser has no annotation tokens, so it keeps a one-slot equivalent of
clang's `annot_decltype`: `annotate_decltype_scope` (at a
`decltype(...)::` found by `decltype_scope_end`, never `decltype(auto)`)
parses the decltype-specifier, calls `Sema::act_on_nested_name_decltype`
and records the result at the `::` (replay depth and index; cleared by
every `bump`). `scan_name_qual_at` and `parse_nested_name_nns` start
from a pending annotation; a failed one (diagnosed "'decltype(i)' (aka
'int') is not a class, namespace, or enumeration") makes the name
unclassifiable, and the rest of the qualified name is skipped silently
(`skip_failed_decltype_scope`), as clang recovers. The decision points
that look at the current token annotate first: the decl-specifier loop
(after a type specifier the form starts a member-pointer declarator),
primary expressions, `is_cxx_declaration_statement`,
`is_cxx_condition_declaration`, `is_cxx_type_id` and template
arguments. A `decltype(e)::name` seen by lookahead at another index (the
tentative scanner after the `(` of a possible cast or `sizeof`, template
argument skipping) is skipped syntactically (`skip_qualified_rest`) and
classified by what follows the name: an identifier, a `*`/`&`/`&&`
before `)`/`,`/`>`, or a `)` before an identifier, literal or `(` make it
a type. A name begins at the `decltype` keyword (`name_begin`).
**Landed (WP-140).** GNU asm ([dcl.asm], `asm.rs`): the asm-declaration
`asm ( string-literal ) ;` at namespace scope goes to
`act_on_file_scope_asm`; in a function body `asm`/`__asm`/`__asm__` with
the qualifiers `volatile`, `inline`, `goto` (duplicates diagnosed,
`const`/`restrict` ignored with clang's warnings) and the extended form
`( template : outputs : inputs : clobbers [: labels] )`, operands
`[name] "constraint" ( expression )`, goes to `act_on_gcc_asm_stmt`. A
wide, Unicode or empty literal is diagnosed at the following token, as
clang does. `asm goto` is parsed and then reported as
`'asm goto' is not supported yet` (callbr lowering is not implemented).
Not implemented: `asm goto`, MS-style `__asm { }` blocks.

**Landed (WP-140).** The predefined identifiers `__func__`,
`__FUNCTION__` and `__PRETTY_FUNCTION__` are keywords and primary
expressions ([dcl.fct.def.general]/8) passed to `act_on_predefined_expr`.

- **Other declarations:**
  - pack expansions in dynamic exception specifications
  - `auto` deduction from braced lists (`std::initializer_list`)
  - `typename`, `operator` and pack forms of using-declarators, and an
    attributed `[[...]] using` as a block-scope statement
  - `asm goto`, MS `__asm` blocks, modules, `__declspec`

## 10. Tests
- **Per-rule tests** in `crates/kiln_parse/tests/`, each tagged with
  its standard section:
  - `decl.rs`: [dcl.*], [class.*], [namespace.*], [basic.def.odr],
    [basic.link]
  - `class.rs`: class heads, member declarations, special
    declarator-ids, bit-fields, `= default`/`= delete`,
    ctor-initializers, missing `;` after a definition, an unterminated
    class
  - `expr.rs`: [expr.*], [lex.string], [lex.ext]
  - `stmt.rs`: [stmt.*]
  - `sema.rs`: rules that need the parser in front of Sema
  - `tokens.rs`: the token source: macro-expanded tokens, alternative
    tokens, pragmas, module directives, and window/whole-buffer parity
  - `common/mod.rs` provides `parse(src) -> Parsed` with
    `assert_clean`, `assert_diag(substr)` and `assert_dump(substr)`.
- **Sequence tests** in `class.rs`'s `#[cfg(test)]` module: the parser
  records each `act_on_*` call of a class in `Parser::trace` (test
  builds only), and the tests pin the order (§4.1). The same module
  tests capture and replay (§2.4).
- **Conformance tests:** `diag:` tests under
  `tests/conformance/{dcl.*,expr.*,stmt.*,basic.*,class.*,namespace.*}/`.
  Tests of the token source are under `cpp.pragma`, `cpp.pragma.op`,
  `cpp.include` (with headers in `Inputs/`), `cpp.replace`,
  `lex.digraph`, `lex.string.uneval` and `module.import`.
  They pass with both `cargo xtask test` and
  `cargo xtask test --compiler clang++-19`.
- **AST goldens:** `tests/conformance/ast/` (spec/ast-dump.md),
  generated with `cargo xtask clang-ast`.
