# WP-071b progress (CTAD, M3 scope)

Status: complete, ready for review. Branch `wp/071b-ctad-guides`, merged with main a9b245d.
Design and what is deferred: spec/sema.md §4.3.2.

| deliverable | status |
|---|---|
| 1. parse deduction guides + AST node | done: namespace and class scope, explicit, requires-clause, template and non-template |
| 2. sema: guide set, overload resolution, diagnostics, sites | done for the M3 scope (spec/sema.md §4.3.2) |
| 3. tests in `tests/conformance/templates/ctad/` | 12 tests (10 pass, 2 xfail: P1816, P1814); all 12 pass under clang++-19 |
| 4. gate-header probe | see below |
| 5. spec + this file | spec/sema.md §4.3.2; pointer from spec/sema-templates.md §6.6 |

## Tests

| file | covers |
|---|---|
| `over.match.class.deduct-var-forms.cpp` | `X x(a)`, `X x{a}`, `X x = a`, `X x = {a}`, `X x;`, cv-qualified |
| `over.match.class.deduct-functional-cast.cpp` | `X(a)`, `X{a}`, `X()` |
| `over.match.class.deduct-new.cpp` | `new X(a)`, `new X{a}`, `new X` |
| `temp.deduct.guide-explicit.cpp` | explicit guides and ctors skipped in copy-init, used in direct-init |
| `temp.deduct.guide-explicit-copy-list-diag.cpp` | explicit ctor/guide selected in copy-list-init (error + note) |
| `over.match.best-user-guide-beats-implicit.cpp` | [over.match.best]/2.10 |
| `over.match.class.deduct-copy-guide.cpp` | copy deduction candidate, /2.11 |
| `over.match.class.deduct-no-viable-diag.cpp` | no viable guide: error, candidate notes, implicit-guide notes |
| `over.match.class.deduct-dependent.cpp` | dependent var/cast/list/copy-init/new re-deduced per instantiation; non-dependent use in a template |
| `temp.deduct.guide-parse.cpp` | guide forms at namespace and class scope |
| `over.match.class.deduct-aggregate.cpp` | xfail: P1816 |
| `over.match.class.deduct-alias.cpp` | xfail: P1814 |

## Fixes outside CTAD found on the way

- A paren-initialized class variable in a template (`S s(1);`, non-dependent) lost its `)`,
  so instantiation redid it as default-initialization ("default-initialization without a
  default constructor is not supported yet"). `CtorCall::rparen` now keeps the end
  location on the `CXXConstructExpr`.
- With class template names now valid placeholders, a class template name as a template
  argument (`__make_integer_seq<S, int, 3>`) is parsed as a template template argument
  first (`parse_template_argument`).

## Probe

`ulimit -v 8388608; timeout 120 kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0 h.cpp`
(error counts).

| header | base f9e7061 | main a9b245d | this branch (merged) |
|---|---:|---:|---:|
| `<utility>` | 6 | 1 | 0 |
| `<array>` | 25 | 8 | 2 |
| `<string_view>` | 41 | 23 | 15 |
| `<span>` | — | 13 | 4 |
| `<compare>` | — | 0 | 0 |

The branch removes every guide error (pair.h:484, subrange.h:197-210, array:384,
string_view:694/699, span:604-620) and adds none. Remaining blockers, all outside CTAD:
- `__datasizeof` keyword (datasizeof.h:31)
- `consteval` (WP-133)
- `__builtin_constant_p`, `__is_assignable`, `__builtin_va_list`
- literal operators (string_view:906)
- local classes in templates (hash.h:270)
- qualified `operator()` call (hash.h:434/444/485)
- switch on a dependent `_Size` (hash.h:62)
- `__invoke` ambiguity (find.h:48)
- `__assign_trivially_copyable` overloads (constexpr_c_functions.h:221)
- the `__integral_constant_like` concept: `bool_constant<static_cast<…>(_Tp()) == …>` (span:595)
