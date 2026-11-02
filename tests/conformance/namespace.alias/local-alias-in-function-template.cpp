// [namespace.alias]/1, [temp.inst]: a namespace alias declared in the body
// of a function template (or a member function template of a class
// template) is redeclared in each instantiation's block scope, so a
// qualified call through it resolves at instantiation; an alias of a
// local alias maps to the instantiation's. libstdc++ <variant>:1546
// (`namespace __variant = std::__detail::__variant;` in
// `variant::emplace`, reached from <generator>'s `ranges::elements_of`).
// Verified against clang 19.
//@ expect: run
//@ stdout: 7 12 3
#include <cstdio>
namespace outer::inner {
template <int N, class T, class... A> int put(T&, A... a) { return (N + ... + a); }
int twice(int x) { return 2 * x; }
}
template <class... Ts> struct V {
  template <int N, class... A> int emplace(A&&... a) {
    namespace in = outer::inner;
    return in::put<N>(*this, static_cast<A&&>(a)...);
  }
};
template <class T> T f(T x) {
  namespace a = outer;
  namespace b = a::inner;
  return b::twice(x);
}
template <class T> auto g(T x) {
  namespace in = outer::inner;
  return [x] { return in::twice(x) / 2; }();
}
int main() {
  V<int> v;
  std::printf("%d %d %d\n", v.emplace<4>(1, 2), f(6), g(3));
}
