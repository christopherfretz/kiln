// [expr.prim.lambda.closure]/7, [temp.inst]/17: the constraints of a
// generic lambda's call operator in a template (a type-constraint and a
// requires-clause) and a non-generic lambda's trailing requires-clause are
// checked with the enclosing instantiation's arguments.
//@ expect: run
template <class T> concept Small = sizeof(T) <= 4;
template <class T> constexpr int f(T v) { return [](Small auto x) { return int(x) + 1; }(v); }
static_assert(f(3) == 4);
template <class T> constexpr int g(T v) {
  auto l = [](auto x) { return 1; };
  auto m = [](Small auto x) { return 2; };
  return l(v) + m('c');
}
static_assert(g(1.0) == 3);
template <class T> constexpr bool h(T) { return requires { [](Small auto x) { return x; }(T()); }; }
template <class T> constexpr int k(T v) { return []<class U>(U u) requires (sizeof(U) > 1) { return 5; }(v); }
static_assert(k(1) == 5);
template <class T> constexpr int n(T v) { return [v]() requires (sizeof(T) <= 4) { return int(v) + 3; }(); }
static_assert(n(4) == 7);
int main() { return f(1) + g(2) + k(1) + n(1) - 14; }
