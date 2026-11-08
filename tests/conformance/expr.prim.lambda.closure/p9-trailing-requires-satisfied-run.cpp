// [expr.prim.lambda.closure]/9, [expr.prim.req.general]/5: a lambda's
// trailing requires-clause in a template is checked after instantiation,
// so a requires-expression that calls it, converts it to a pointer to
// function with unary '+', or calls a generic one is false when the
// constraint is not; when it is satisfied the conversion to pointer to
// function is usable and runs the body. Covers function templates and
// members of class templates. Verified against clang 19 (GCC 16.1 agrees).
//@ expect: run
template <class U> constexpr bool g1() { return requires { [](auto) requires(sizeof(U) > 1) {}(0); }; }
static_assert(g1<long>() && !g1<char>());
template <class U> constexpr bool g2() { return requires { +[](int) requires(sizeof(U) > 1) { return 1; }; }; }
static_assert(g2<long>() && !g2<char>());
template <class U> constexpr int g3() { int (*p)(int) = [](int x) requires(sizeof(U) > 1) { return x; }; return p(3); }
static_assert(g3<long>() == 3);
template <class U> struct S {
  static constexpr bool v = requires { [](int) requires(sizeof(U) > 1) {}(0); };
  constexpr int m() { return [](int x) requires(sizeof(U) > 1) { return x + 1; }(1); }
};
static_assert(S<long>::v && !S<char>::v && S<long>{}.m() == 2);
template <class U> constexpr bool g4() { return requires { [](U x) requires(sizeof(x) > 1) {}(U{}); }; }
static_assert(g4<long>() && !g4<char>());
int main() { return g3<int>() - 3; }
