// Mangling differential: closures in a function template's signature
// called with explicit template arguments (spec/sema-lambda.md §3.5,
// spec/abi.md §8.15, WP-1086; [expr.prim.lambda.closure]/2). A non-empty
// explicit list substitutes the parameter types and the return type,
// leading or trailing, once per call (clang
// `SubstituteExplicitTemplateArguments`), and forming a new
// specialization substitutes the signature again: each substitution
// numbers the signature's lambdas per <lambda-sig> in the pattern. So
// `ret2<int>` (new) is `UlvE0_`, its repeat takes one number, `ret2<char>`
// is `UlvE2_`; `f<>` and a deduced-only call number once. clang 19 and 22
// agree on every name here.
template <class T> auto ret2() -> decltype([] { return 1; }) { return {}; }
template <class T> decltype([] { return 1; }) lead() { return {}; }
// A partial explicit list and deduction; the parameter named in the
// trailing return type; a deduced call between explicit ones.
template <class T, class U> auto part(U) -> decltype([] { return 1; }) { return {}; }
template <class T> auto named(T x) -> decltype(x, [] { return 1; }) { return {}; }
template <class T> auto ded(T) -> decltype([] { return 1; }) { return {}; }
// Packs: wholly and partially explicit, empty `<>`, a pack the trailing
// return type expands.
template <class... Ts> auto pk(Ts...) -> decltype([] { return 1; }) { return {}; }
template <class... Ts> auto pk2(Ts...) -> decltype([] { return 1; }) { return {}; }
template <class T, class... Ts> auto pk3(T, Ts...) -> decltype([] { return 1; }) { return {}; }
template <class... Ts> auto pp(Ts... ts) -> decltype((ts, ...), [] { return 1; }) { return {}; }
// Two lambdas, of one <lambda-sig> and of two; a <lambda-sig> naming T.
template <class T> auto two(T) -> decltype([] { return 1; }(), [] { return 2; }) { return {}; }
template <class T> auto twod(T) -> decltype([](int) { return 1; }(0), [] { return 2; }) { return {}; }
template <class T> auto dep(T) -> decltype([](T) { return 1; }) { return {}; }
// A lambda in a parameter type, with a trailing and a leading return.
template <class T> auto pt(T, decltype([] { return 3; }) c = {}) -> int { return c(); }
template <class T> int ptl(T, decltype([] { return 3; }) c = {}) { return c(); }
// A default template argument's lambda (a new type per use, numbered in
// the namespace) beside a trailing return type's.
template <class T, class U = decltype([] { return 2; })>
auto dtr(T) -> decltype([] { return 1; }) { return {}; }
template <class T, class U = decltype([] { return 2; })> auto dtu(T) -> U { return {}; }
// Member templates, `this` in the trailing return type, a static one.
struct S {
  int m = 0;
  template <class T> auto f(T) -> decltype([] { return 1; }) { return {}; }
  template <class T> auto g(T) -> decltype(this->m, [] { return 1; }) { return {}; }
  template <class T> static auto h() -> decltype([] { return 1; }) { return {}; }
};
// A lambda nested in the signature's lambda; declared, then defined.
template <class T> auto nest(T) -> decltype([] { return [] { return 1; }; }()) { return {}; }
template <class T> auto fwd(T) -> decltype([] { return 1; });
// Address-of forms that resolve the template-id once, as a call does.
template <class T> struct is_int { static constexpr bool v = false; };
template <> struct is_int<int> { static constexpr bool v = true; };
template <class T, class L> auto pick(L l) {
  if constexpr (is_int<T>::v) return 0; else return l;
}
template <class T> auto fa(T) -> decltype(pick<T>([] { return 1; })) { return {}; }
template <class T> auto fb(T) -> decltype(pick<T>([] { return 1; })) { return {}; }
template <class T> auto fc(T) -> decltype(pick<T>([] { return 1; })) { return {}; }
int use() {
  S s;
  int r = ret2<int>()() + ret2<int>()() + ret2<char>()() + lead<int>()() + lead<int>()()
        + lead<char>()();
  r += part<int>(1.0)() + part<int>(1)() + named<int>(1)() + named<long>(1)();
  r += ded(1)() + ded<int>(1)() + ded(2L)();
  r += pk<int>(1, 2)() + pk<int, int>(1, 2)() + pk(1)() + pk2<>(1)() + pk2(1)();
  r += pk3<int>(1, 2)() + pk3<int, long>(1, 2)() + pp<int>(1, 2L)() + pp<int>(1, 2.0)();
  r += two<int>(1)() + two<long>(1)() + twod<int>(1)() + twod<long>(1)();
  r += dep<int>(1)(1) + dep<long>(1)(1) + dep(1)(1) + dep(1.0)(1);
  r += pt<int>(1) + pt<long>(1) + pt(1) + pt(1.0) + ptl<int>(1) + ptl<long>(1) + ptl(1) + ptl(1.0);
  r += s.f<int>(1)() + s.f<long>(1)() + s.g<int>(1)() + s.g<long>(1)() + S::h<int>()()
     + S::h<long>()();
  r += dtr<int>(1)() + dtr<int>(1)() + dtr<long>(1)() + dtu<int>(1)() + dtu<int>(1)()
     + dtu<long>(1)();
  r += nest<int>(1)() + nest<long>(1)() + fwd<int>(1)();
  (void)fa<int>;
  int (&rb)(int) = fb<int>;
  int (*pc)(int) = fc;
  return r + fa<long>(1)() + rb(1) + fb<long>(1)() + pc(1) + fc<long>(1)();
}
template <class T> auto fwd(T) -> decltype([] { return 1; }) { return {}; }
