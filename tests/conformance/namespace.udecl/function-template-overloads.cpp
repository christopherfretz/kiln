// [namespace.udecl]/1, [basic.scope.scope]/4, [temp.over.link]/7: a
// using-declarator naming an overload set of function templates
// introduces every member; function templates with different signatures
// (or a function template and a function) do not conflict. libc++
// `pair::swap`'s `using std::swap;` (`__utility/pair.h:454`). WP-128.
//@ expect: compile-ok

namespace s {
template <class T> constexpr void swap(T& a, T& b) { T t = a; a = b; b = t; }
template <class T, unsigned long N> constexpr void swap(T (&a)[N], T (&b)[N]) {
  for (unsigned long i = 0; i != N; ++i) swap(a[i], b[i]);
}
template <class T> struct P {
  T a, b;
  constexpr void swap(P& p) {
    using s::swap;
    swap(a, p.a);
    swap(b, p.b);
  }
};
} // namespace s
namespace b { template <class T> int h(T*) { return 1; } }
namespace c { template <class T> int h(T) { return 2; } inline int h(int) { return 3; } }

constexpr int f() {
  s::P<int> x{1, 2}, y{3, 4};
  x.swap(y);
  int u[2] = {5, 6}, v[2] = {7, 8};
  using s::swap;
  swap(u, v);
  return x.a * 1000 + y.b * 100 + u[0] * 10 + v[1];
}
static_assert(f() == 3000 + 200 + 70 + 6);

int k() {
  using b::h;
  using c::h;
  int i = 0;
  return h(&i) * 100 + h(1.0) * 10 + h(1);
}

int main() { return k() == 123 ? 0 : 1; }
