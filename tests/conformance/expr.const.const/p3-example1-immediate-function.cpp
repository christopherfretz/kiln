// [expr.const.const]/3 (P4101R1) Example 1, the lines on pointers to
// immediate functions: `a` is an error (a non-constexpr variable holding
// a consteval-only value, /1.2), `b` and `c` are OK (a constexpr
// variable, a template parameter object), as /4 no longer restricts the
// result of a constant expression. kiln keeps the pre-P4101R1 rejection
// of `b` and `c` until WP-1328 land 2 (P3603 constexpr pointers to
// consteval functions); GCC 16.1 and clang 22 reject `b` too.
//@ expect: compile-fail
//@ xfail(kiln): b and c are WP-1328 land 2 (constexpr pointers to immediate functions)
//@ skip(clang): clang implements the pre-P4101R1 permitted-result rule
//@ strict-diags
//@ diag: 19:10 error "cannot take address of consteval function 'plus1' outside of an immediate invocation"
//@ diag: 14:15 note "declared here"
consteval int plus1(int x) { return x + 1; }
template <auto V> struct C {};

constexpr auto b = plus1;               // OK
auto c = C<plus1>();                    // OK
auto a = plus1;                         // error
