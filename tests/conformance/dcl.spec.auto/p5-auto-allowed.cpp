// [dcl.spec.auto.general]/2-6, [expr.new]/2, [temp.param]/2,
// [expr.prim.req.general]/4: positions where a placeholder is allowed, or
// where a trailing return type replaces the `auto` that would otherwise be
// rejected (alias, typedef, cast, template argument, requires parameter).
//@ expect: compile-ok
template<class T> struct S {};
template<class T> concept C = true;
int inc(int x) { return x; }
using F = auto(*)(int)->int;
typedef auto F2(int) -> int;
S<auto(*)(int)->int> s1;
auto p1 = static_cast<auto(*)(int)->int>(inc);
auto n1 = new (auto)(1);
auto n2 = new auto(1);
template<auto N> struct V {};
template<C auto N> struct V2 {};
template<auto (*fp)(int) -> auto> struct E {};
auto f1() -> auto { return 1; }
decltype(auto) f2() { return 1; }
auto l1 = []() -> decltype(auto) { return 1; };
auto l2 = [](auto x) { return x; };
bool r1 = requires (auto (*p)(int) -> int) { p; };
struct Q {
  operator auto() { return 1; }
  friend auto g() -> int;
  static constexpr auto z = 1;
};
void h(int (&arr)[2]) {
  for (auto x : arr) (void)x;
  if (auto y = 1) (void)y;
  auto [a, b] = arr;
  C auto c = 1;
  const auto &d = c;
  (void)a; (void)b; (void)d;
}
void k(auto (*p)(int) -> int);
template<class T = int> struct D {};
