// Debug info (WP-1365, [namespace.udir], [namespace.udecl], [namespace.alias]):
// block-scope using-directives, using-declarations and namespace aliases
// are scoped to the innermost lexical block (or the subprogram) and kept
// in the subprogram's `retainedNodes`, not in the compile unit's
// `imports:`; also in a function template's instantiation, an inline
// function, a lambda, and after a `return` (clang emits declarations of
// unreachable code).
//@ args: -g
namespace N {
int v = 1;
int get();
struct P {
  int q;
};
} // namespace N
template <class T> T tf(T t) {
  using namespace N;
  using N::v;
  return t + v;
}
inline int il() {
  using N::get;
  namespace Q = N;
  return get() + Q::v;
}
int caller(int a) {
  int local = a;
  using N::P;
  P p{local};
  {
    using namespace N;
    namespace R = N;
    {
      using N::v;
      local += v + R::v;
    }
  }
  int l = [](int x) {
    using N::v;
    return x + v;
  }(local);
  return tf(p.q) + il() + l;
}
int after_return() {
  return 0;
  using namespace N;
}
int N::get() { return 7; }
