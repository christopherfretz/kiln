// [namespace.udecl]/1 (P0195): the `overloaded` idiom: using-declarator
// pack expansions of operator() and of a named member, with CTAD over
// closure types.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 5 6 3 4 10 20
extern "C" int printf(const char *, ...);
template<class... Ts> struct overloaded : Ts... { using Ts::operator()...; };
template<class... Ts> overloaded(Ts...) -> overloaded<Ts...>;
struct A { int operator()(int) const { return 1; } };
struct B { int operator()(double) const { return 2; } };
template<class... Ts> struct V : Ts... { using Ts::operator()...; using Ts::f...; };
struct F1 { int f(int) { return 10; } int operator()(char) const { return 3; } };
struct F2 { int f(double) { return 20; } int operator()(long) const { return 4; } };
int main() {
  overloaded o{A{}, B{}};
  auto l = overloaded{[](int) { return 5; }, [](const char *) { return 6; }};
  V<F1, F2> v;
  printf("%d %d %d %d %d %d %d %d\n", o(1), o(1.0), l(1), l("x"), v('c'), v(1L), v.f(1), v.f(1.0));
}
