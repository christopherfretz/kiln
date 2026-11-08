// [dcl.spec.auto.general]/4 [temp.arg.explicit]/4 [over.over]: a class
// member access whose template-id names a single static member function
// specialization deduces a placeholder like the function lvalue itself
// (clang `resolveAndFixSingleFunctionTemplateSpecialization`): `auto`
// and `const auto` a pointer, `auto&`/`decltype(auto)` a function
// reference; `&g.B<1>` is a function pointer. spec/sema-overload.md
// "Member overload sets", WP-788 (WP-823).
//@ expect: run
//@ stdout: 1 2 3 4 5 6 7
extern "C" int printf(const char *, ...);
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
struct G {
  template <int N> static int B() { return N; }
  template <class T> int B(int);
  template <int N> static int Is() { return N; }
};
int main() {
  G g;
  auto q = g.Is<3>;
  decltype(auto) a = g.B<1>;
  auto &b = g.B<2>;
  auto &&c = g.B<3>;
  const auto d = g.B<4>;
  auto e = &g.B<5>;
  auto f = G::B<6>;
  static_assert(same<decltype(q), int (*)()>);
  static_assert(same<decltype(a), int (&)()>);
  static_assert(same<decltype(b), int (&)()>);
  static_assert(same<decltype(c), int (&)()>);
  static_assert(same<decltype(d), int (*const)()>);
  static_assert(same<decltype(e), int (*)()>);
  static_assert(same<decltype(f), int (*)()>);
  printf("%d %d %d %d %d %d %d\n", a(), b(), c(), d(), e(), f(), q() + 4);
}
