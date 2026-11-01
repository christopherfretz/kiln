# WP-134 progress (CTAD follow-ups to WP-071b)

Status: implementation complete, in final merge/CI. Branch `wp/134-ctad-followups`.
Design and what is deferred: spec/sema.md §4.3.2; summary in spec/sema-templates.md §6.6.

| scope item | status |
|---|---|
| 1. constructor templates as implicit guides | done (`ctor_template_guide`) |
| 2. tie-breakers [over.match.best.general]/2.11-2.13 | done (`overload/best.rs`) |
| 3. `CXXDeductionGuideDecl` AST dump | done (user guides; implicit ones dropped, as clang) |
| 4. P1816 aggregate deduction | done; xfail flipped |
| 4. P1814 alias-template deduction | done; xfail flipped |
| 5. P2582 inherited-constructor guides | deferred: clang 19 lacks it (kiln matches clang 19 today) |
| 5. member class template guides after instantiation | done for non-template ctors and member guides, via an alias (`O::In`); ctor templates and `Outer<int>::In` (parse) deferred |
| 5. array `new` with CTAD | done: clang's "cannot form array of deduced class template specialization type" |
| 6. phase-1 braced CTAD | done (initializer-list guides first; single-specialization exception) |

## Tests (`tests/conformance/templates/ctad/`, all matched against clang++-19)

| file | covers |
|---|---|
| `over.match.class.deduct-ctor-template.cpp`, `-ctor-template-diag.cpp` | guides from constructor templates, constraints, notes |
| `over.match.best-ctor-beats-ctor-template.cpp` | /2.13 |
| `temp.deduct.guide-ast-dump.cpp` | AST dump of user guides |
| `over.match.class.deduct-aggregate.cpp`, `-aggregate-diag.cpp` | P1816 (was xfail) |
| `over.match.class.deduct-alias.cpp`, `-alias-diag.cpp` | P1814 (was xfail): packs, NTTPs, user guides, `__is_deducible` |
| `over.match.class.deduct-member-template.cpp`, `-member-template-diag.cpp` | member templates of `Outer<int>` |
| `over.match.class.deduct-new-array-diag.cpp` | `new X[n]{…}` |
| `over.match.class.deduct-init-list.cpp`, `-init-list-diag.cpp` | [over.match.list] phase 1 |

## Follow-ups found

- `Outer<int>::In x(…)` (template-id qualifier) parses `In` as a dependent name: a variable of
  dependent type, no diagnostic (kiln_parse name classification).
- P2582 (with the clang 22 oracle switch).
- Deduction notes print `const char[N]` where clang prints `char[N]` ("could not match").
- Default arguments are copied unsubstituted into ctor-template / member guides.
- Printing keeps typedef args in `N::S<N::L>`; the ambiguous multi-base `o(1)` is not
  diagnosed; `using Ts::operator()...` is unsupported (seen in alias/overload probes).
- `E<int> e{{1, 2}}` (nested list into an initializer-list ctor of a class): "this
  list-initialization of a class is not supported yet" (init, not CTAD).
