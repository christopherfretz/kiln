// Captureless lambdas: closure types, call operators, the conversion to
// function pointer and its static invoker, and closure mangling ($_N for
// lambdas in non-inline contexts, Ul...E_ numbered in inline ones).
struct Big { long a, b, c, d; };
struct Pair { int x; int y; };

int use(int v) {
  auto sq = [](int x) { return x * x; };
  int (*fp)(int) = sq;
  return sq(v) + fp(v) + [] { return 1; }();
}

auto glob = [](int a) noexcept { return a + 1; };
int g2() { return glob(2); }

inline int inl() {
  return [](int q) { return q; }(5) + [](long q) { return (int)q; }(6) +
         [](int q) { return q * 2; }(7);
}
int g3() { return inl(); }

auto st = []() static { return 9; };
int g4() {
  int (*p)() = st;
  return st() + p();
}

Big g5(long n) {
  auto mk = [](long a, const Big& tail) { return Big{a, tail.b, tail.c, a}; };
  Big (*fp)(long, const Big&) = mk;
  Big t{1, 2, 3, 4};
  return fp(n, t);
}

Pair g6(int n) {
  auto mk = [](int a, Pair p) { return Pair{a + p.x, p.y}; };
  Pair (*fp)(int, Pair) = mk;
  return fp(n, Pair{3, 4});
}

int counter;
void g7() {
  auto bump = [](int& r) { ++r; };
  void (*fp)(int&) = bump;
  fp(counter);
  bump(counter);
}

inline auto iv = [](int x) { return x - 1; };
int g8() { return iv(3); }

struct S {
  static inline auto z = [] { return 4; };
};
int g9() { return S::z(); }

int g10() {
  struct L {
    int h() { return 2; }
  };
  L l;
  return l.h();
}
