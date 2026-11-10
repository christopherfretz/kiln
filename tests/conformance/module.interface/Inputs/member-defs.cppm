// Exported classes whose members are defined out of class without
// `export` ([module.interface]/1, /6; WP-1159).
export module member_defs;

export template <auto P> struct X {
  static int x;
  int f() const;
  template <class T> int g(T) const;
  struct M;
};
template <auto P> int X<P>::x = P;
template <auto P> int X<P>::f() const { return P + 1; }
template <auto P> template <class T> int X<P>::g(T t) const { return int(t) * P; }
template <auto P> struct X<P>::M { int v = P * 10; };

export struct S {
  static int x;
  int f() const;
  template <class T> T g(T) const;
  struct N;
  enum class E : int;
};
int S::x = 7;
int S::f() const { return x * 2; }
template <class T> T S::g(T t) const { return t + T(1); }
struct S::N { int a = 3; };
enum class S::E : int { a = 4 };

export namespace ns {
int h();
}
export int ns::h() { return 5; }
