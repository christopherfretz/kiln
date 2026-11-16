// [expr.prim.splice]/3, /4, [temp.names]/3: `template [: r :]` without
// template arguments designates a function template's overload set
// (here and after substitution, as a call operand and in a member
// access); with arguments it designates a function or variable template
// specialization. Found running GCC's C++ testsuite.
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
template <typename T> constexpr int twice(T t) { return 2 * t; }
template <typename T> constexpr T forty = 40;
struct S {
  template <typename T> constexpr int get(T t) const { return t + 1; }
};

static_assert(template [: ^^twice :](21) == 42);
static_assert(template [: ^^twice :]<long>(4) == 8);
static_assert(template [: ^^forty :]<int> == 40);

template <decltype(^^int) R> constexpr int call(const S &s) {
  return template [: R :](3) + s.template [: ^^S::get :](1);
}
static_assert(call<^^twice>(S{}) == 8);

template <decltype(^^int) R> constexpr int member(const S &s) {
  return s.template [: R :](5);
}
static_assert(member<^^S::get>(S{}) == 6);
