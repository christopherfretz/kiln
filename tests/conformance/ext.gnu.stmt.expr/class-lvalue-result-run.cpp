// [ext.gnu.stmt.expr] A statement expression whose last expression is a
// class lvalue copy-initializes its result object from it with the copy
// constructor, inside the statement expression's scope, so the copy is
// made before its locals are destroyed (clang `InitializeStmtExprResult`;
// spec/sema.md §5.8). The same holds in constant evaluation. WP-799.
//@ expect: run
//@ strict-diags
//@ stdout: 3 6|copy 7|dtor 7|s 107|n 5|dtor 107|ce 101021
extern "C" int printf(const char *, ...);

struct P {
  int a, b;
};

struct S {
  int v;
  S(int x) : v(x) {}
  S(const S &o) : v(o.v + 100) { printf("copy %d|", o.v); }
  ~S() { printf("dtor %d|", v); }
};

struct C {
  int v;
  int *log;
  constexpr C(int x, int *l) : v(x), log(l) {}
  constexpr C(const C &o) : v(o.v + 100), log(o.log) { ++*log; }
  constexpr ~C() { *log += 10; }
};

constexpr P mk(int x) { return ({ P r{x, 2 * x}; r; }); }
constexpr int field(int x) { return ({ P r{x, 2}; r; }).b; }
constexpr int counted() {
  int n = 0;
  {
    C c = ({ C r(1, &n); r; });
    n += c.v * 1000;
  }
  return n;
}
static_assert(mk(3).a == 3 && mk(3).b == 6);
static_assert(field(3) == 2);
static_assert(counted() == 101021);

int main() {
  volatile int three = 3;
  P p = mk(three);
  printf("%d %d|", p.a, p.b);
  {
    S s = ({ S t(7); t; });
    int n = ({ P r{5, 0}; r; }).a;
    printf("s %d|n %d|", s.v, n);
  }
  printf("ce %d\n", counted());
}
