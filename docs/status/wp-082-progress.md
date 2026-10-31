# WP-082 progress: defaulted comparison operators

Branch `wp/082-defaulted-comparisons`, worktree `.worktrees/wp-082`. Main
was merged at e990962 (WP-106, WP-100, WP-116).

## Status: complete, pending the full CI
- Categories: `std::*_ordering` lookup and validation, plus lookup by
  name. The built-in `<=>` works in sema, the Oven and codegen.
- Declaration checks, deletedness, category deduction, constexpr, and the
  implicit `==`.
- Clang-shaped bodies, defined on odr-use. The implicit exception spec
  comes from the body.
- Class templates: member and friend defaulted comparisons.
- Spec: sema-class.md §4.4, §4.12, §8.3 "Landed (WP-082)" and §11 Q12
  (gaps); sema-overload.md §10.6; sema-templates.md §4.8; types.md R4.
- Tests: `class.compare.default/`, `class.eq/`, `class.spaceship/`,
  `class.compare.secondary/`, `cmp.categories/`, plus four ast goldens.
- The synthesized three-way comparison reports `not supported yet`. It
  needs a PseudoObjectExpr; its test is xfailed (Q12).

## Probe (`kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0`)

| header | baseline (45fbbab) | main e990962 | wp-082 |
|---|---|---|---|
| `<compare>` | 44 | 15 | 6 |
| `<utility>` | 77 | 32 | 23 |

These count `: error:` lines. All WP-082-owned errors are gone. The ones
left in `<compare>` belong to range-for (WP-089), lambdas (WP-075), and
the cascades those cause.

## Next step
Run the full `cargo xtask ci`, then hand back.
