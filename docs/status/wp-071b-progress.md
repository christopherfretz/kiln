# WP-071b progress (CTAD, M3 scope)

Status at supervisor stop request (2026-10-31). No compiler code has changed yet;
the work so far is investigation, clang 19 reference capture, and the probe baseline.

| deliverable | status |
|---|---|
| 1. parse deduction guides + AST node/dump | not started (design done, see below) |
| 2. sema: guide set, overload resolution, diagnostics, hooks | not started (design done) |
| 3. tests in `tests/conformance/templates/ctad/` (>= 8) | not started |
| 4. gate-header probe | baseline ("before") recorded; "after" pending |
| 5. `spec/sema.md` CTAD section + this file | this file only |

## Probe baseline (before)

`ulimit -v 8388608; timeout 120 kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0 h.cpp`
on branch base f9e7061:

| header | exit | errors | guide-related errors |
|---|---:|---:|---|
| `<utility>` | 1 | 6 | `__utility/pair.h:484` |
| `<array>` | 1 | 25 | pair.h:484, `__ranges/subrange.h:197,200,203,210`, `array:384` |
| `<string_view>` | 1 | 41 | pair.h:484, subrange.h:197-210, `string_view:694,699` |

Non-CTAD blockers seen (not in scope, list only): `__do_test` lookup
(sfinae_helpers.h:44), braced args in a dependent call, using-declaration
conflicts (pair.h:454/462, `__fwd/subrange.h:43`), void return in `<new>`,
`::new` placement in construct_at.h, `::__builtin_memmove`, consteval,
`__builtin_va_list`, `__builtin_constant_p`, `__is_assignable`, literal
operators, local classes in templates, `hash.h:434` qualified
`operator()` call, `__invoke` ambiguity.

## Clang 19 reference behaviour (captured)

- Guide dump: `CXXDeductionGuideDecl <deduction guide for W> 'auto (const char *) -> W<int>'`;
  template guides are `FunctionTemplateDecl <deduction guide for W>` with
  `implicit_instantiation` specialization children; implicit guides are
  `implicit` FunctionTemplateDecls at TU level.
- Deduced vars: `VarDecl s 'N::S<long>' callinit`, `CXXFunctionalCastExpr 'N::S<long>'
  functional cast to N::S <ConstructorConversion>`, `CXXNewExpr 'N::S<long> *'`.
- No viable guide: `error: no viable constructor or deduction guide for deduction of
  template arguments of 'N::S'` at the declarator name, then per candidate the normal
  template note plus `note: implicit deduction guide declared as 'template <class T> S(T, T) -> S<T>'`
  (copy guide: `'template <class T> S(S<T>) -> S<T>'`).
- Explicit guide in copy-list-init: `error: class template argument deduction for 'E'
  selected an explicit deduction guide for copy-list-initialization` + `note: explicit
  deduction guide declared here`. In plain copy-init (`E e = 1;`) explicit guides are
  simply not candidates.

## Design (to implement next)

Parser: `SpecialName::DeductionGuide(DeclId)`; `at_special_declarator` detects
`[explicit] TemplateName ( ... ) ->`; base type `auto` so the trailing return applies;
`parse_type_name_spec` / tentative parser / `parse_name_primary` / new-expression accept
a bare class-template name as the `DeducedTemplateSpecialization` placeholder.
Sema (`templ/ctad.rs`): per class template, cache synthesized implicit guide
FunctionTemplates (one per primary ctor, injected-class-name -> `X<T...>`, return
`X<T...>`), plus the copy guide `X(X<T...>) -> X<T...>`, plus user guides found by
looking up the guide name in the template's context. Resolve with
`add_template_candidate`/`best_viable`; exclude explicit guides under CopyInit; add the
sema-overload.md §4.1 item 10 tie-breakers in `best.rs::better`. Deduced type = return
type of the winning specialization. Dependent initializers keep the placeholder and
redo CTAD at instantiation. Ctor templates (re-depthing), aggregate CTAD (P1816), and
alias CTAD (P1814) are deferred. Normalizer: `CXXDeductionGuideDecl` -> Keep and drop
guide specializations (`xtask/src/clang_ast/table.rs:172`).
