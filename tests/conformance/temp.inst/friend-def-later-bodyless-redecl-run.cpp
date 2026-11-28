// [temp.inst]/4, [class.friend]/7, [basic.def.odr]: a friend function defined
// in a class template specialization is instantiated when odr-used, whichever
// redeclaration the call binds — including when a later bodyless friend
// declaration (in another base specialization, in the same class template, or
// at namespace scope after an earlier bodyless friend) is the latest
// redeclaration in the chain, or when the call binds a bodyless redeclaration
// before the defining specialization is instantiated (reduced from real code,
// WP-3686).
//@ expect: run
//@ std: c++20
struct X {
  int mf(int) { return 1; }
  int mf(long) { return 2; }
};

// Reproducer: B<X> defines fr, A<X> redeclares fr without a body.
struct D;
template <class T> struct B {
  friend int fr(X u, D *) { return static_cast<T &>(u).mf(1L); }
};
template <class T> struct A {
  friend int fr(X u, D *);
};
struct D : B<X>, A<X> {};
int w() { return fr(X(), (D *)nullptr); }

// (a) Control: bodyless friend in Aa<X>, then definition in Ba<X>.
struct Da;
template <class T> struct Ba {
  friend int fr_a(X u, Da *) { return static_cast<T &>(u).mf(1L); }
};
template <class T> struct Aa {
  friend int fr_a(X u, Da *);
};
struct Da : Aa<X>, Ba<X> {};
int w_a() { return fr_a(X(), (Da *)nullptr); }

// (b) One class template holding the definition and then a bodyless redecl
// (both via ADL and via a namespace-scope declaration).
struct Dc;
template <class T> struct C {
  friend int g(X u) { return static_cast<T &>(u).mf(1L); }
  friend int g(X u);
  friend int g_adl(X u, Dc *) { return static_cast<T &>(u).mf(1L); }
  friend int g_adl(X u, Dc *);
};
int g(X u);
struct Dc : C<X> {};
int w_b() { return g(X()) + g_adl(X(), (Dc *)nullptr); }

// (c) Definition in a class template, bodyless redeclaration at namespace scope
// after the instantiation.
struct Dns;
template <class T> struct Bns {
  friend int fr_ns(X u, Dns *) { return static_cast<T &>(u).mf(1L); }
};
struct Dns : Bns<X> {};
int fr_ns(X u, Dns *);
int w_c() { return fr_ns(X(), (Dns *)nullptr); }

// (d) Call binds a bodyless friend redeclaration before the specialization
// holding the definition is instantiated (both pure ADL and namespace-scope).
struct Dd1;
template <class T> struct Ad1 {
  friend int fr_d1(X u, Dd1 *);
};
template <class T> struct Bd1 {
  friend int fr_d1(X u, Dd1 *) { return static_cast<T &>(u).mf(1L); }
};
struct Dd1 : Ad1<X> {};
int w_d1() { return fr_d1(X(), (Dd1 *)nullptr); }
Bd1<X> bd1_inst;

struct Dd2;
template <class T> struct Ad2 {
  friend int fr_d2(X u, Dd2 *);
};
template <class T> struct Bd2 {
  friend int fr_d2(X u, Dd2 *) { return static_cast<T &>(u).mf(1L); }
};
Ad2<X> ad2_inst;
int fr_d2(X u, Dd2 *);
int w_d2() { return fr_d2(X(), (Dd2 *)nullptr); }
Bd2<X> bd2_inst;

// (e) Bodyless friend is the first declaration, defining friend is second,
// namespace-scope bodyless redeclaration is third (found by call).
struct De;
template <class T> struct Ae {
  friend int fr_e(X u, De *);
};
template <class T> struct Be {
  friend int fr_e(X u, De *) { return static_cast<T &>(u).mf(1L); }
};
struct De : Ae<X>, Be<X> {};
int fr_e(X u, De *);
int w_e() { return fr_e(X(), (De *)nullptr); }

int main() {
  if (w() != 2) return 1;
  if (w_a() != 2) return 2;
  if (w_b() != 4) return 3;
  if (w_c() != 2) return 4;
  if (w_d1() != 2) return 5;
  if (w_d2() != 2) return 6;
  if (w_e() != 2) return 7;
  return 0;
}
