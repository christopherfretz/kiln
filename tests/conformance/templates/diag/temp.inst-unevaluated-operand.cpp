// [expr.context]/1, [expr.prim.req.general]/2, [temp.inst]/5: the operands
// of sizeof, alignof, noexcept and the requirements of a requires-expression
// are unevaluated, also when rebuilt at instantiation; a function template
// they name is not odr-used, so its definition is not instantiated (libc++'s
// std::declval static_asserts in its body).
//@ expect: compile-ok
//@ strict-diags
template <class T> T &&dv() noexcept { static_assert(!__is_same(T, T), "evaluated"); }
template <class F, class To>
concept conv = __is_convertible(F, To) && requires { static_cast<To>(dv<F>()); };
static_assert(conv<int, long>);
template <class T> requires requires { dv<T>() + 1; } constexpr int f() { return 1; }
static_assert(f<short>() == 1);
template <class T> constexpr bool g = requires { { dv<T>() } noexcept; typename T::x; };
static_assert(!g<char>);
template <class T> constexpr int h() {
  return sizeof(dv<T>()) + noexcept(dv<T>()) + __is_lvalue_expr(dv<T>()) +
         requires { dv<T>(); };
}
static_assert(h<double>() == 10);
template <class T> struct S { static constexpr unsigned long n = sizeof(dv<T>()); };
static_assert(S<long>::n == 8);
int main() {}
