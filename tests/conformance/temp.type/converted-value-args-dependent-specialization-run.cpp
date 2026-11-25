// [temp.type]/1, [temp.over.link]/5, [temp.mem.func]/1: the non-dependent
// non-type arguments of a dependent class template-id are converted to
// their parameters' types, so `It<T, 1 == 1>` and `It<T, true>` are one
// type (as are `Q<T>` and `Q<T, 1u>` for a `long` parameter defaulted to
// 1, an enumerator and its cast value, `nullptr` and `(int *)0`, `&x`
// and `(&x)`, `P{1}` and `P{0 + 1}`). A declaration and a definition
// spelling them differently redeclare one function template, and an
// out-of-line member defined as `It<T, 1 == 1>::f` is the member of the
// partial specialization `It<T, true>` (WP-3321). Reduced from real code.
//@ expect: run
//@ stdout: 1 2 3 4 5 6 7 8 9 10 11 12 0
extern "C" int printf(const char *, ...);

enum E { e0, e1 };
int x;
struct P {
  int a;
};
template <class T, bool B> struct It {
  static int id() { return 0; }
};
template <class T> struct It<T, true> {
  void f();
  static int g();
  int h(It<T, 1 == 1> *);
};
template <class T> void It<T, 1 == 1>::f() {}
template <class T> int It<T, true>::g() { return 6; }
template <class T> int It<T, !false>::h(It<T, true> *) { return 7; }
template <class T, long N = 1> struct Q {};
template <class T, int M, unsigned N> struct Z {};
template <class T, E e> struct A {};
template <class T, int *p> struct Ptr {};
template <class T, P v> struct G {};

template <class T> int k(It<It<T, 1 == 1>, true>);
template <class T> int k(It<It<T, true>, 1>) { return 1; }
template <class T> int q(Q<T>);
template <class T> int q(Q<T, 1u>) { return 2; }
template <int M, class T> int z(Z<T, M, 3>);
template <int M, class T> int z(Z<T, M, 1 + 2>) { return 3; }
template <class T> using Al = It<T, 1 == 1>;
template <class T> int a(Al<T>);
template <class T> int a(It<T, true>) { return 4; }
template <class T> struct O {
  template <class U, bool C> struct In {};
  static int m(In<T, true>);
};
template <class T> int O<T>::m(In<T, 1 == 1>) { return 5; }
template <class T> int en(A<T, e1>);
template <class T> int en(A<T, E(1)>) { return 8; }
template <class T> int np(Ptr<T, nullptr>);
template <class T> int np(Ptr<T, (int *)0>) { return 9; }
template <class T> int ad(Ptr<T, &x>);
template <class T> int ad(Ptr<T, (&x)>) { return 10; }
template <class T> int cl(G<T, P{1}>);
template <class T> int cl(G<T, P{0 + 1}>) { return 11; }
template <class T> struct Fr {
  friend class It<T, 1 == 1>;
  static int get(It<T, true> *) { return 12; }
};

int main() {
  It<int, true> v;
  v.f();
  int r1 = k(It<It<int, true>, true>{});
  int r2 = q(Q<int, 1>{});
  int r3 = z(Z<int, 4, 3>{});
  int r4 = a(It<int, true>{});
  int r5 = O<int>::m({});
  int r6 = It<int, true>::g();
  int r7 = v.h(nullptr);
  int r8 = en(A<int, e1>{});
  int r9 = np(Ptr<int, nullptr>{});
  int r10 = ad(Ptr<int, &x>{});
  int r11 = cl(G<int, P{1}>{});
  int r12 = Fr<int>::get(nullptr);
  int r0 = It<int, false>::id();
  printf("%d %d %d %d %d %d %d %d %d %d %d %d %d\n", r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11,
         r12, r0);
}
