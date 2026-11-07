// [temp.arg.explicit]/4, [over.call.func]/3, [expr.prim.paren]: a
// parenthesized function template-id that is called, `(f<A>)(x)` or
// `((f<A>))(x)`, is the same overload set as the unparenthesized one: the
// call deduces the remaining template arguments, a trailing parameter
// pack included. It does not name the specialization with only the
// explicit arguments ([over.over]/3 applies to a template-id not called),
// so that specialization is not instantiated. Reduced from
// google/benchmark v1.9.5's BENCHMARK_CAPTURE((F<A, B>), name, args...).
//@ expect: run
//@ exit: 0
template <class T, class U, class... E> int count(int, E... e) {
  // Ill-formed for the specialization with E empty.
  static_assert(sizeof...(E) != 0, "E deduced from the call");
  return static_cast<int>(sizeof...(E));
}

namespace ns {
template <class T, class... E> int count2(E...) {
  static_assert(sizeof...(E) != 0, "E deduced from the call");
  return static_cast<int>(sizeof...(E));
}
} // namespace ns

template <class T> int first(T t) { return static_cast<int>(t); }

template <class T> int in_template(T t) {
  return (count<T, char *>)(0, t, t) + ((ns::count2<T>))(t);
}

int main() {
  int a = (count<void, char *>)(0, 42u);           // E = {unsigned}
  int b = ((count<void, char *>))(0, 1, 2.0, 'c'); // E = {int, double, char}
  int c = (ns::count2<int>)(1L, 2L);               // E = {long, long}
  int d = in_template(7);                          // 2 + 1
  // Not called: the template-id names its single specialization.
  int (*p)(int) = (first<int>);
  int e = (first<int>)(5) + p(1);
  return a == 1 && b == 3 && c == 2 && d == 3 && e == 6 ? 0 : 1;
}
