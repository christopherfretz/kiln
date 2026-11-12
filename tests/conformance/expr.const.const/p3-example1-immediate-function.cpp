// [expr.const.const]/3 (P4101R1) Example 1, the lines on pointers to
// immediate functions: `a` is an error (a non-constexpr variable holding
// a consteval-only value, /1.2), `b` and `c` are OK (a constexpr
// variable, a template parameter object), as /4 no longer restricts the
// result of a constant expression (P3603's constexpr pointers to
// immediate functions, carried by P4101R1; WP-1361). GCC 16.1 and clang
// 22 still reject `b` (docs/status/deviations.md); `a` keeps clang's
// wording.
//@ expect: compile-fail
//@ skip(clang): clang implements the pre-P4101R1 permitted-result rule
//@ strict-diags
//@ diag: 19:10 error "cannot take address of consteval function 'plus1' outside of an immediate invocation"
//@ diag: 14:15 note "declared here"
consteval int plus1(int x) { return x + 1; }
template <auto V> struct C {};

constexpr auto b = plus1;               // OK
auto c = C<plus1>();                    // OK
auto a = plus1;                         // error
