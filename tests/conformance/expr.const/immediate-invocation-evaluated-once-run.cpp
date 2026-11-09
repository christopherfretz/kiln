// [expr.const]/16-17: an immediate invocation is a full constant
// expression evaluated once; the value it produces initializes its result
// object in every context: a local, a constexpr variable, a constinit
// variable, a namespace-scope variable with dynamic initialization, a
// default member initializer, a default argument and the body of an
// immediate-escalating function. kiln stores each invocation's value and
// reuses it as a leaf in the initializer's later folds and in codegen
// (WP-1140): scalars, pointers, class objects with arrays, bases, unions,
// bit-fields and pointers into string literals, and the classes that are
// still re-evaluated (a mutable member, a virtual function).
//@ expect: run
//@ strict-diags
//@ stdout: 17711 6 9 abc 7 3 1 | 17711 6 9 abc 7 | 17711 6 9 abc 7 | 17711 6 9 abc 7 | 17711 9 | 17711 15 15 | 17711 6 | 4 5 | 12
extern "C" int printf(const char*, ...);

consteval long fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }

struct Base { int b = 1; };
struct Table : Base {
  int v[4] = {};
  const char* s = nullptr;
  union { int i; float f; } u = {0};
  unsigned bits : 3 = 0;
};
consteval Table build(int k) {
  Table t;
  for (int i = 0; i < 4; ++i) t.v[i] = i * k;
  t.b = 6;
  t.s = "abc";
  t.u.i = 7;
  t.bits = 3;
  return t;
}

struct M { mutable int m; int n; };
consteval M make_m(int x) { return M{x, x + 1}; }

struct V {
  int x;
  constexpr virtual int get() const { return x; }
};
consteval V make_v(int x) { V v; v.x = x; return v; }

// A namespace-scope variable: constexpr, constinit, and dynamically
// initialized (the invocation is constant, the call around it is not).
constexpr long cx_fib = fib(22);
constexpr Table cx_tab = build(3);
constinit long ci_fib = fib(22);
constinit Table ci_tab = build(3);
long id(long x) { return x; }
Table idt(Table t) { return t; }
long dyn_fib = id(fib(22));
Table dyn_tab = idt(build(3));

struct S {
  long f = fib(22);
  Table t = build(3);
};

long def_fib(long x = fib(22)) { return x; }
int def_tab(Table t = build(3)) { return t.v[3] + t.b; }

template <class T>
constexpr long escal(T k) {
  long a = fib(k);
  Table t = build(k - 17);
  return a + t.v[3] - 15 + t.b;
}

int main() {
  long l_fib = fib(22);
  Table l_tab = build(3);
  M m = make_m(3);
  V v = make_v(1);
  printf("%ld %d %d %s %d %d %d | ", l_fib, l_tab.b, l_tab.v[3], l_tab.s,
         l_tab.u.i, (int)l_tab.bits, m.m == 3 && m.n == 4 && v.get() == 1);
  printf("%ld %d %d %s %d | ", cx_fib, cx_tab.b, cx_tab.v[3], cx_tab.s, cx_tab.u.i);
  printf("%ld %d %d %s %d | ", ci_fib, ci_tab.b, ci_tab.v[3], ci_tab.s, ci_tab.u.i);
  printf("%ld %d %d %s %d | ", dyn_fib, dyn_tab.b, dyn_tab.v[3], dyn_tab.s, dyn_tab.u.i);
  S s;
  printf("%ld %d | ", s.f, s.t.v[3]);
  printf("%ld %d %d | ", def_fib(), def_tab(), def_tab(build(3)) + 0 * def_tab());
  printf("%ld %d | ", escal(22) - 6, cx_tab.b);
  static constexpr Table st = build(1);
  printf("%d %d | ", st.v[3] + 1, st.v[2] + 3);
  constexpr long k = fib(5) + fib(6) - 1;
  printf("%ld\n", k);
  return 0;
}
