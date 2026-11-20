// [expr.sizeof]/1, /5: the operand of `sizeof...` is a parenthesized
// identifier. `sizeof... Ts` is reported where the `(` belongs, with
// fix-its inserting both parentheses, and is otherwise taken as
// `sizeof...(Ts)`; `sizeof...` followed by neither is an error.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ diag: 11:51 error "missing parentheses around the size of parameter pack 'Ts'"
//@ diag: 12:51 error "expected parenthesized parameter pack name in 'sizeof...' expression"
//@ diag: 13:58 error "missing parentheses around the size of parameter pack 'Is'"
template <class... Ts> constexpr int a = sizeof... Ts;
template <class... Ts> constexpr int b = sizeof...;
template <int... Is> constexpr int c() { return sizeof... Is + 1; }
static_assert(a<int, char> == 2);
static_assert(c<1, 2, 3>() == 4);
static_assert(c<>() == 1);
