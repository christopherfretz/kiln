// [class.temporary]/6, [stmt.ranged]/1, [temp.inst]: in an instantiated
// function template every temporary a declaration extends is that
// specialization's own object, destroyed when its declaration's lifetime
// ends, in reverse order of construction: `__range`'s temporary (a
// prvalue range and a member of a temporary), a reference bound to a
// subobject of a temporary, the reference members of an aggregate, a
// structured binding's reference, and a static local reference's, whose
// object is distinct per specialization (one shared object printed `+11`
// twice and destroyed it twice, WP-3271). Reduced from real code.
//@ expect: run
//@ std: c++20
//@ strict-diags
//@ stdout: [ +1 1 2 -1 | +3 3 4 -3 ] +4 +5 +8 +9 ; -9 -8 -5 -4 +10 = 53 10
//@ stdout: [ +1 1 2 -1 | +3 3 4 -3 ] +4 +5 +8 +9 ; -9 -8 -5 -4 +11 = 54 11
//@ stdout: 10 11
//@ stdout: exit: -11 -10 end
extern "C" int printf(const char *, ...);
struct S {
  int v;
  int a[2];
  S(int x) : v(x), a{x, x + 1} { printf(" +%d", v); }
  ~S() { printf(" -%d", v); }
  const int *begin() const { return a; }
  const int *end() const { return a + 2; }
};
struct A {
  const S &s;
  const int &i;
};
struct W {
  S s;
};
struct P {
  S x, y;
};
struct End {
  ~End() { printf(" end\n"); }
} end;

template <class T> const S &f(T base) {
  int n = 0;
  printf("[");
  for (int x : S(1)) {
    n += x;
    printf(" %d", x);
  }
  printf(" |");
  for (int x : W{S(3)}.s) {
    n += x;
    printf(" %d", x);
  }
  printf(" ]");
  {
    const int &i = S(4).a[1];
    A agg{S(5), 6};
    const auto &[p, q] = P{S(8), S(9)};
    n += i + agg.s.v + agg.i + p.v + q.v;
    printf(" ;");
  }
  static const S &st = S(10 + base);
  n += st.v;
  printf(" = %d %d\n", n, st.v);
  return st;
}

int main() {
  const S &a = f<int>(0);
  const S &b = f<long>(1);
  printf("%d %d\nexit:", a.v, b.v);
}
