# WP-142 — M3 gate unblockers 2

Branch `wp/142-gate-unblockers-2`, merged with main at WP-114. The libc++ M3
gate went from 254/585 (WP-141) to 307/585 (`docs/status/libcxx-m3.md`).
Every fix has a conformance test, and each test's expected output was checked
against clang++-19.

## Items

| # | item | status | fix |
|---|---|---|---|
| 1 | `>=` in a template argument (`<tuple>:575`) | fixed | `kiln_parse/src/parser.rs:822` `scan_template_args`: `>=` is a relational operator inside a template argument list, never a closer ([temp.names]/4 splits only `>` and `>>`). `>` inside `()`/`[]`/`{}` nested in an argument is an operator (`greater_ends_arg` :449, `greater_off_depth`), including in `decltype(a > b)` (`<optional>`) |
| 2 | `'…' does not refer to a value` for a member type (`<atomic>`) | fixed | `parser.rs:1065` `annotate_template_scope`: `X<A>::name` whose name the lookahead cannot find in `X`'s primary pattern (a partial or explicit specialization member, a dependent base) is parsed ahead and annotated like `decltype(e)::`; typedefs to specializations work as scopes too |
| 3 | deduced return type used before it is defined (`__get_iter_concept`) | fixed | `kiln_sema/src/deduce.rs:477` `undeduced_dependent_use`: a use of an undeduced member function in a dependent context waits for instantiation ([dcl.spec.auto.general]/11) |
| 4 | anonymous member classes (`basic_string::__short`/`__long`) | fixed | `templ/subst/anon.rs:37` instantiates unnamed member classes and anonymous unions/structs with their members; `:101` copies class attributes (`[[__gnu__::__packed__]]`, `alignas`) |
| 5 | `excess elements in scalar initializer` | fixed | `kiln_sema/src/decl.rs:2849`: `T x(a, b)` with a dependent `T` or argument stays a `ParenListExpr` until instantiation; braced NSDMIs likewise (`class/mod.rs:312`) |
| 6 | explicit instantiation of a static data member (`basic_string::npos`) | fixed | `templ/var.rs:356` `act_on_explicit_var_inst` (static data members and variable templates; `extern template` suppresses emission); codegen `module.rs:1228` `var_instantiation_linkage` (weak_odr / linkonce_odr) |
| 7a | template type parameter must be a type | fixed by item 1/2 | no longer reproduces in the gate |
| 7b | `delete` of a `<dependent type>` | fixed | `new_delete.rs:1235`: a type-dependent operand is kept; `templ/subst/exprs.rs:514` rebuilds it |
| 7c | chrono `typedef int` vs `long long` | not reproduced | `<chrono>` has no such error after items 1–6 |

Found on the way (they blocked the same headers):

- Out-of-line members of class template partial specializations
  (`templ/mod.rs:645` `match_template_lists_to_scope`, `templ/actions.rs:245`
  `current_instantiation`).
- `X<P...>::` names the current instantiation even when lookup finds a friend
  redeclaration of `X` with deeper parameters (libc++ `weak_ptr`).
- Default template arguments added by a class template redeclaration
  (`templ/mod.rs:743` `inherit_redeclared_defaults`), and template arguments
  checked against the head declaration's parameters (`:772` `head_template`), not a
  friend redeclaration's deeper ones. libc++ `__wrap_iter` befriends
  `basic_string` before `<string>` declares it, so the explicit
  instantiations in `<string>` matched nothing in every TU that includes
  `<array>` or `<span>` first. This was the third-largest first-error
  cluster (19 tests).
- Member class templates of non-template classes: their parameters were
  undeclared in late-parsed member bodies, default arguments and NSDMIs
  (`kiln_parse/src/class.rs:567` `replay_phase`). libc++ `<variant>`
  `__dispatcher<_Is...>::__dispatch` hit this, and it was most of the
  `use of undeclared identifier` first errors.
- The injected-class-name in out-of-line members; pack expansions in
  functional casts; alias templates of enclosing packs; member function
  parameter packs; the class being defined in default arguments.
- Codegen: static data members of implicit instantiations were emitted as
  `external global` (`module.rs:428` `instantiated_var_def`); members of
  dependent contexts are skipped.

## Headers

Errors with `-ferror-limit=0` (libc++ 19 headers, `-std=c++26`); `<memory>`, `<optional>` and `<atomic>` are
now down to the blockers listed below:

| header | WP-141 | WP-142 |
|---|---:|---:|
| `<string>` | 87 | 0 |
| `<memory>` | 52 | 6 |
| `<vector>` | 622 | 60 |
| `<chrono>` | 876 | 428 |
| `<optional>` | 20 | 3 |
| `<atomic>` | 14 | 14 |

## Divergences from clang

- An explicit instantiation of a static data member does not check the
  declared type against the member's type.
- An explicit instantiation of an undeclared `w<int>` adds a second
  diagnostic (`expected ';'`).
- A non-odr-used `constexpr` static member read in an NSDMI is loaded from
  its global rather than folded (the result is the same).

## Leftover blockers

Ranked by how many headers they block. Each repro was checked against clang++-19, which accepts it.

1. **Templates that differ only in an `enable_if` default NTTP are treated as
   redeclarations** (WP-138). This gives "constructor cannot be redeclared"
   (`<optional>:685/695/705`, `shared_ptr.h:261/648`) and "redefinition of"
   for the out-of-line definitions (`vector:1218`). Overloaded members then
   become ambiguous (`shared_ptr.h:286` `__on_zero_shared_impl`). Repro:
   `template <class X = A, EI_t<sizeof(typename X::value_type) == 4, int> = 0> explicit S(A);`
   next to the same declaration with `!= 4`, inside `template <class A> struct S`.
2. **`using B::operator=;`** (28 first errors in the gate; every `<variant>`
   user, through `_LIBCPP_VARIANT_DESTRUCTOR` at `variant:769`). This is
   rejected at `kiln_parse/src/decl.rs:378` as "this kind of name in a
   using-declarator is not supported yet". It is in the WP-114 area, which this
   WP did not touch:
   `struct B { B& operator=(int); }; struct D : B { using B::operator=; };`
3. **Friend class template specialization** (WP-136; 12 errors in `<vector>`,
   23 in `<chrono>`):
   `template <class T> struct X; template <class T> struct Y { friend struct X<T>; }; Y<int> y;`
4. **Braced default arguments** (10 in `<vector>`, 16 in `<chrono>`):
   `struct P { int a; }; int g(P p = {}) { return p.a; }`
5. **Builtins** (WP-144): `__builtin_labs`, `__builtin_fabsf`, `__atomic_add_fetch`, `__atomic_always_lock_free`,
   `__c11_atomic_*`, `__builtin_va_start`, `__builtin_trunc`,
   `__builtin_round` and `__builtin_fpclassify`.
6. **`_Atomic(T)` as a type specifier** (`cxx_atomic_impl.h:274`). Every
   remaining `<atomic>` "no member named `__a_value`" error follows from it:
   `template <class T> struct A { _Atomic(T) a; }; A<int> x;`
7. **Local using-directive in a template** (`convert_to_timespec.h:30`, and so
   every `<chrono>`/`<atomic>`/`<thread>` user):
   `namespace n { int g(int); } template <class T> T f(T t) { using namespace n; return g(t); }`
8. **Explicit object parameters** (`format_arg.h:292`):
   `struct S { int v; int get(this const S& s) { return s.v; } };`
9. In `<chrono>`, after the above: `override` on members of a class template
   whose virtual base members come from a dependent base (`locale:2087`, 14
   errors). There are also undeclared members of dependent bases such as
   `__split_buffer::__first_` (11). These are probably downstream of item 1
   and still need triage.
