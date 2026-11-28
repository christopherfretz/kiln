// [temp.inst]/4, [class.friend]/7, [basic.def.odr]: C++17 variant of
// friend-def-later-bodyless-redecl-run.cpp (reduced from real code, WP-3686).
//@ expect: run
//@ std: c++17
struct X {
  int mf(int) { return 1; }
  int mf(long) { return 2; }
};

struct D;
template <class T> struct B {
  friend int fr(X u, D *) { return static_cast<T &>(u).mf(1L); }
};
template <class T> struct A {
  friend int fr(X u, D *);
};
struct D : B<X>, A<X> {};
int w() { return fr(X(), (D *)nullptr); }

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
  if (w_b() != 4) return 2;
  if (w_e() != 2) return 3;
  return 0;
}
