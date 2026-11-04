// [namespace.udecl]/14, [temp.over.link]/6-7 (C++20): a constrained
// member function template hides a using-declared base one only when
// the template heads (type-constraints, requires-clauses) and trailing
// requires-clauses are equivalent; `k` differs in its trailing
// requires-clause, so both stay candidates and satisfaction picks one.
// Here base and derived are both class template specializations. WP-612.
//@ expect: run
//@ strict-diags
//@ args: -std=c++20
template <class T> concept Small = sizeof(T) <= 4;
template <class T> concept Big = sizeof(T) > 4;
template <class U> struct B {
  template <class O> requires Small<O> int f(O) { return 1; }
  template <Small O> int g(O) { return 1; }
  template <class O> int h(O) requires Small<O> { return 1; }
  template <class O> int k(O) requires Small<O> { return 1; }
};
template <class T> struct D : B<T> {
  using B<T>::f;
  using B<T>::g;
  using B<T>::h;
  using B<T>::k;
  template <class O> requires Small<O> int f(O) { return 2; }
  template <Small O> int g(O) { return 2; }
  template <class O> int h(O) requires Small<O> { return 2; }
  template <class O> int k(O) requires Big<O> { return 3; }
};
struct P {
  template <class O> requires Small<O> int f(O) { return 1; }
};
struct Q : P {
  using P::f;
  template <class O> requires Small<O> int f(O) { return 2; }
};
int main() {
  D<int> d;
  if (d.f(1) != 2 || d.g(1) != 2 || d.h(1) != 2) return 1;
  if (d.k(1) != 1 || d.k(1.0) != 3) return 2;
  Q q;
  if (q.f(1) != 2) return 3;
  return 0;
}
