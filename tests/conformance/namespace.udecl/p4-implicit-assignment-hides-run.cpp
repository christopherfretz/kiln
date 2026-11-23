// [namespace.udecl]/4 and /14: a base assignment operator a
// using-declarator brings in that has the signature of the derived class's
// copy or move assignment operator is hidden by the implicitly-declared one
// ([class.copy.assign]/2, /4), which is declared when the class is
// completed: whether the class's record marks the base one hidden does not
// depend on a later use of the assignment (WP-3146). `a = b` and
// `a = std::move(b)` call the implicit members (memberwise: `v` copied);
// `a = 7` reaches the base's `operator=(int)` through the
// using-declarator.
//@ expect: run
//@ stdout: 2 3 7 | 5 7
extern "C" int printf(const char *, ...);
struct D;
struct B {
  int v = 0;
  B &operator=(const D &) { v = 100; return *this; }
  B &operator=(D &&) { v = 200; return *this; }
  B &operator=(int x) { v = x; return *this; }
};
struct D : B {
  using B::operator=;
};
// Never used: its record is decided at `}` all the same.
struct D2;
struct B2 {
  B2 &operator=(const D2 &);
};
struct D2 : B2 {
  using B2::operator=;
};
struct M;
struct N {
  int v = 0;
  N &operator=(M &&) { v = 300; return *this; }
  N &operator=(int x) { v = x; return *this; }
};
struct M : N {
  using N::operator=;
};
template <class T> T &&mv(T &x) { return static_cast<T &&>(x); }
int main() {
  D a, b;
  b.v = 2;
  a = b;
  int r1 = a.v;
  b.v = 3;
  a = mv(b);
  int r2 = a.v;
  a = 7;
  int r3 = a.v;
  printf("%d %d %d | ", r1, r2, r3);
  M m, n;
  n.v = 5;
  m = mv(n);
  int r4 = m.v;
  m = 7;
  printf("%d %d\n", r4, m.v);
}
