// Local classes of function templates and of member functions of class
// templates, instantiated with the enclosing function: their members and
// the Itanium local names (Z <function encoding> E <entity>) of their
// member functions, with discriminators for same-named local classes.
typedef unsigned long size_t;
template <class T> size_t bits(T v) {
  union { T t; struct { size_t a; size_t b; } s; } u;
  u.s.a = 0;
  u.s.b = 0;
  u.t = v;
  return u.s.a ^ u.s.b;
}
template <class T, int N> int local(T v) {
  struct L {
    T x;
    L(T w) : x(w + N) {}
    T get() const { return x * N; }
  };
  int r = 0;
  { struct M { int f() { return 1; } }; r += M().f(); }
  { struct M { int f() { return 2; } }; r += M().f(); }
  return int(L(v).get()) + r;
}
template <class T> struct C {
  int f(int k) const {
    struct D { int m; int g(int q) const { return m + q + int(sizeof(T)); } };
    return D{k}.g(1);
  }
};
int use() { return int(bits(1.5)) + local<int, 2>(1) + local<char, 3>('a') + C<short>().f(4); }
