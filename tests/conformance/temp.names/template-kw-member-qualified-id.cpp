// [expr.ref]/1, [basic.lookup.qual.general], [temp.names]/5, CWG1835: after
// `.` or `->` an optional `template` and a nested-name-specifier whose
// components are template-ids (`d.template B<T>::f()`,
// `d.template D<T>::template B<T>::i`, `x.B<int>::template g<char>()`),
// with dependent and non-dependent object types, a base-class qualifier
// found as the injected-class-name of the base ([temp.local]/1), and a
// destructor name (GCC 16.1, clang 22). Found running GCC's C++ testsuite
// (WP-1466).
//@ expect: run
//@ std: c++20
//@ strict-diags
template<class T> struct B { int foo() { return 1; } int i; template<class U> U g() { return U(2); } ~B() {} };
template<class T> struct D : B<T> { };
struct N : B<int> { };
template<class T> int fn(D<T> d, D<T> *p) {
  int r = d.template B<T>::foo();
  d.template B<T>::i = 3;
  r += d.template D<T>::template B<T>::i;
  r += p->template B<T>::foo();
  r += d.B<T>::template g<int>();
  r += D<T>().template D<T>::template B<T>::foo();
  return r;
}
int nd(N n, N *p) {
  int r = n.template B<int>::foo() + p->B<int>::template g<char>();
  r += n.N::template B<int>::foo();
  p->template B<int>::~B();
  return r;
}
template<class T> void pd(T *p) { p->template B<int>::~B(); }
int main() {
  D<int> d; N n;
  int r = fn(d, &d) + nd(n, &n);
  B<int> *q = new B<int>; pd(q); ::operator delete(q);
  return r == 1 + 3 + 1 + 2 + 1 + 1 + 2 + 1 ? 0 : 1;
}
