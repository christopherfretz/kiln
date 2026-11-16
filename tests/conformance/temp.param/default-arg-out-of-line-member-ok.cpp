// [temp.param]/12: defaults given in the class are used by out-of-line
// member definitions without them; a non-member template and a member
// template of a non-template class may add one (the sibling
// default-arg-out-of-line-member rejects the class template members).
//@ std: c++20
//@ expect: run
template <typename T = int> struct A {
  struct B;
  void f();
  template <class U = int> void g();
  template <class U = int> struct C;
  static int v;
};
template <typename T> struct A<T>::B { int i = 1; };
template <typename T> void A<T>::f() {}
template <typename T> template <class U> void A<T>::g() {}
template <typename T> template <class U> struct A<T>::C { U u = 2; };
template <typename T> int A<T>::v = 3;
template <int N = 4> struct X;
template <int N> struct X { int n = N; };
struct P { template <class U> int k(); };
template <class U = long> int P::k() { return sizeof(U); }
int main() {
  A<> a;
  a.f();
  a.g();
  A<>::B b;
  A<>::C<> c;
  X<> x;
  P p;
  return b.i + c.u + A<>::v + x.n + (p.k<char>() == 1) == 11 ? 0 : 1;
}
