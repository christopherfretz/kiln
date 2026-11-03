// [dcl.constexpr] (C++23, P2448R2): a constexpr function may define a
// variable of non-literal type, in a template and in a lambda too; only
// evaluating the definition is not a constant expression ([expr.const]).
// The C++20 counterpart is non-literal-local-template-cxx20.cpp. WP-414.
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
struct B { B(); ~B(); };
// Function templates: a non-dependent variable at the definition; a
// dependent one never (t3<B> is silently not constexpr).
template<class T> constexpr int t1() { B x; return 0; }
template<class T> constexpr int t2() { B x; return 0; }
int u2 = t2<int>();
template<class T> constexpr int t3() { T x; return 0; }
int u3 = t3<B>() + t3<int>();
template<class T> constexpr int t4(T) { B x; return 0; }
int t4(...);
int u4 = t4(1);
// Members of a class template; member templates.
template<class T> struct S {
  constexpr int m() { B x; return 0; }
  constexpr int n() { T x; return 0; }
};
int u5 = S<int>().m();
int u6 = S<B>().n();
struct C { template<class T> constexpr int m() { B x; return 0; } };
int u7 = C().m<int>();
// Lambdas: only one declared constexpr (or consteval) explicitly.
auto l1 = []() constexpr { B x; return 0; };
auto l2 = [] { B x; return 0; };
auto l3 = [](auto) constexpr { B x; return 0; };
auto l4 = [](auto y) constexpr { decltype(y) x; return 0; };
int u8 = l2() + l4(B()) + l4(1);
int u9 = l3(1);
template<class T> constexpr auto tl() { return [] { T x; return 0; }(); }
int u10 = tl<B>();
