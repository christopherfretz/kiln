// [dcl.struct.bind]/4: a tuple-like initializer without a member `get`
// is initialized by `get<i>(xvalue)` looked up by argument-dependent
// lookup only. That call is the implementation's, not a written
// template-id, so C++17 gives no "use of function template name with no
// prior declaration ... is a C++20 extension" warning ([temp.names]/3
// applies to written names; clang builds it with BuildCallExpr).
// Verified against clang 19.
//@ std: c++17
//@ expect: run
//@ strict-diags
namespace n {
struct P {
  int a, b;
};
template <unsigned long I> int get(const P& p) { return I == 0 ? p.a : p.b; }
} // namespace n
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
template <> struct tuple_size<n::P> {
  static constexpr unsigned long value = 2;
};
template <unsigned long I> struct tuple_element<I, n::P> {
  using type = int;
};
} // namespace std
int main() {
  n::P p{3, 4};
  auto [x, y] = p;
  n::P ps[1] = {{5, 6}};
  int s = 0;
  for (auto [u, v] : ps) s += u * 10 + v;
  return x == 3 && y == 4 && s == 56 ? 0 : 1;
}
