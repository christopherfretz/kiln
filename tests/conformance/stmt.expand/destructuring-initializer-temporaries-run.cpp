// [class.temporary]/9 (CWG3043), [stmt.expand]/5.3, [expr.context]/2: a
// destructuring statement's expansion-initializer is a lifetime-extending
// context, so a discarded class prvalue in it (the operand of a cast to
// void, a comma's left operand) is materialized and, like every other
// temporary created there, persists until the end of the whole statement;
// so do the temporaries of the default arguments it uses
// ([intro.execution]/5), and in a template, where the initializer is
// substituted. An lvalue initializer binds the reference to no temporary
// of its own; its argument temporaries still persist. At run time and in
// constant evaluation. GCC 16.1 agrees; clang 22 has no P1306. WP-1149;
// spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ stdout: a1 a2 ~D1 | b3 b4 ~D2 | c5 c6 ~D3 | e1 e2 ~D7 | f1 f2 ~D9 ~D8 | t1 t2 ~D10 | t3 t4 ~D11 | t1 t2 ~D7 | t1 t2 ~D20 | t3 t4 ~D21 | t1 t2 ~D7 | 45v|34c|end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
struct P { int a, b; };
struct T2 { int a, b; };
constexpr P f() { return {1, 2}; }
constexpr const T2& id(const T2& t) { return t; }
const P& pin(const D&, const P& p) { return p; }
P mk(const D& = D{7}) { return {1, 2}; }
int dv(const D& d = D{8}) { return d.v; }
P mkn(int = dv(), const D& = D{9}) { return {1, 2}; }

template <class U> void tpl(U u) {
  template for (auto x : ((void)D{u}, f())) printf("t%d ", x);
  printf("| ");
  template for (auto x : id(T2{((void)D{u + 1}, 3), 4})) printf("t%d ", x);
  printf("| ");
  template for (auto x : mk()) printf("t%d ", x);
  printf("| ");
}

struct L {
  char buf[16]{};
  int n = 0;
  constexpr void put(char c) { buf[n++] = c; }
};
struct LW {
  L* l;
  char c;
  constexpr ~LW() { l->put(c); }
};
constexpr L run() {
  L l;
  template for (auto x : ((void)LW{&l, 'v'}, P{4, 5})) l.put(char('0' + x));
  l.put('|');
  template for (auto x : id(T2{((void)LW{&l, 'c'}, 3), 4})) l.put(char('0' + x));
  l.put('|');
  return l;
}
constexpr L k = run();
static_assert(k.buf[2] == 'v' && k.buf[6] == 'c');

int main() {
  template for (auto x : ((void)D{1}, f())) printf("a%d ", x);
  printf("| ");
  template for (auto x : id(T2{((void)D{2}, 3), 4})) printf("b%d ", x);
  printf("| ");
  template for (auto x : pin(D{3}, P{5, 6})) printf("c%d ", x);
  printf("| ");
  template for (auto x : mk()) printf("e%d ", x);
  printf("| ");
  // A default argument's own default argument.
  template for (auto x : mkn()) printf("f%d ", x);
  printf("| ");
  tpl(10);
  tpl(20);
  printf("%send\n", k.buf);
}

// Default arguments that bind references: the extended temporaries are what
// the result's members refer to.
struct RP { const int& a; const int& b; };
constexpr RP mkr(const int& a = int{1}, const int& b = int{2}) { return {a, b}; }
constexpr int g() {
  int s = 0;
  template for (auto x : mkr()) s += x;
  return s;
}
static_assert(g() == 3);
