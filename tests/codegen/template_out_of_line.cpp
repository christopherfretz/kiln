// Members of class template specializations defined out of line
// ([temp.mem], [temp.mem.func], [temp.static], [temp.mem.class]) and
// explicitly specialized ([temp.expl.spec]). WP-117: their definitions
// used to be emitted from the pattern as `_ZN1A1gEv`; an explicitly
// specialized static data member is a strong definition, implicit ones
// are linkonce_odr. Also: LLVM's module-wide struct-name counter
// (`%struct.A.0`, with `.base` names of empty non-POD classes taking
// values), copies of empty classes emit nothing, a trivially
// default-constructed local is still destroyed, and a reference-returning
// call in an rvalue context loads through the reference.
template <class T> struct A {
  void g();
  static int s;
  static T st;
  struct N {
    void f();
    static int ns;
  };
  template <class U> struct B {
    void h();
    void h2(T, U);
    template <class V> void deep(V, U, T);
  };
  template <class U> void m(U);
  template <class U> U conv(T, U *);
  A();
  A(T, int);
  ~A();
  int operator+(int);
  bool operator==(const A &) const;
  T &operator[](unsigned);
  void operator()(T, T);
  operator T() const;
  A &operator=(const A &);
  void cq() const &;
  void rq() &&;
  static void sf(T *);
  typedef T value_type;
  value_type vt(value_type);
  A *self(A &, const A *);
};
template <class T> void A<T>::g() {}
template <class T> int A<T>::s = 3;
template <class T> T A<T>::st;
template <class T> void A<T>::N::f() {}
template <class T> int A<T>::N::ns = 4;
template <class T> template <class U> void A<T>::B<U>::h() {}
template <class T> template <class U> void A<T>::B<U>::h2(T, U) {}
template <class T>
template <class U>
template <class V>
void A<T>::B<U>::deep(V, U, T) {}
template <class T> template <class U> void A<T>::m(U) {}
template <class T> template <class U> U A<T>::conv(T, U *) { return U(); }
template <class T> A<T>::A() {}
template <class T> A<T>::A(T, int) {}
template <class T> A<T>::~A() {}
template <class T> int A<T>::operator+(int) { return 0; }
template <class T> bool A<T>::operator==(const A &) const { return true; }
template <class T> T &A<T>::operator[](unsigned) { return st; }
template <class T> void A<T>::operator()(T, T) {}
template <class T> A<T>::operator T() const { return T(); }
template <class T> A<T> &A<T>::operator=(const A &) { return *this; }
template <class T> void A<T>::cq() const & {}
template <class T> void A<T>::rq() && {}
template <class T> void A<T>::sf(T *) {}
template <class T> typename A<T>::value_type A<T>::vt(value_type v) { return v; }
template <class T> A<T> *A<T>::self(A &, const A *) { return this; }

namespace ns {
template <class T, int N> struct C {
  void f(C *);
  static const int k;
  template <class U> struct D {
    void g(D *, C *);
  };
};
template <class T, int N> void C<T, N>::f(C *) {}
template <class T, int N> const int C<T, N>::k = N;
template <class T, int N> template <class U> void C<T, N>::D<U>::g(D *, C *) {}

template <template <class> class TT> struct H {
  void f(TT<int>);
};
template <template <class> class TT> void H<TT>::f(TT<int>) {}

template <class T> struct P;
template <class T> struct P<T *> {
  void f(T);
  static int ps;
};
template <class T> void P<T *>::f(T) {}
template <class T> int P<T *>::ps = 5;
} // namespace ns

template <> void A<long>::g() {}
template <> int A<long>::s = 9;

int use() {
  A<int> a;
  a.g();
  A<int>::s = 1;
  A<int>::st = 1;
  A<int>::N().f();
  A<int>::N::ns = 2;
  A<int>::B<char>().h();
  A<int>::B<char>().h2(1, 'c');
  A<int>::B<char>().deep(1.0, 'c', 1);
  A<int>::B<A<int>>().h2(1, A<int>());
  a.m(1.0);
  a.m(a);
  a.conv(1, (double *)nullptr);
  A<int> a2(1, 2);
  (void)(a + 1);
  (void)(a == a2);
  a[0u];
  a(1, 2);
  int i = 0;
  a = a2;
  a.cq();
  A<int>().rq();
  A<int>::sf(nullptr);
  a.vt(3);
  a.self(a, &a);
  A<double> d;
  d.g();
  d.m(d);
  A<A<int>> aa;
  aa.g();
  aa.self(aa, nullptr);
  A<long>().g();
  A<long>::s = 2;
  ns::C<int, 3> c;
  c.f(&c);
  ns::C<int, 3>::D<char>().g(nullptr, &c);
  ns::C<A<int>, -1>().f(nullptr);
  (void)ns::C<int, 3>::k;
  ns::H<A> hh;
  hh.f(A<int>());
  ns::P<int *>().f(1);
  ns::P<int *>::ps = 6;
  return i + A<int>::s;
}

struct E { ~E(); };
struct F {};
void takeE(E);
void takeF(F);
void empties() {
  E e;
  takeE(e);
  E e2 = e;
  e2 = e;
  F x;
  takeF(x);
  F y = x;
  y = x;
}

template <class T> struct W {};
template <class T> struct V { void m(); V &operator=(const V &); };
void names(V<int> &a, V<double> &b, V<long> &c) {
  W<int> w1;
  a.m();
  b.m();
  c.m();
  W<char> w2;
}

int &ref();
int refcall() { return ref() + 1; }
void takeInt(int);
void refcall2() { takeInt(ref()); }
