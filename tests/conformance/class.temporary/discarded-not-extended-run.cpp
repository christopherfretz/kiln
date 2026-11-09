// [class.temporary]/4, /6, [expr.context]/2, [expr.static.cast]/6,
// [expr.comma]/1: a discarded class prvalue (the operand of a cast to
// void, a comma's left operand) in an ordinary declaration's initializer
// is destroyed at the end of that full-expression: only a reference
// binding extends a temporary, and the element-temporary rule of
// expansion statements ([class.temporary]/8) does not apply. So `~D1`
// and `~D2` (reverse order of construction) come before `y3`, also for a
// reference, a block-scope static, and a class-type variable or
// reference initialized from the comma (kiln's codegen once let the
// variable's slot claim the left operand's temporary and never destroyed
// it). The constant evaluator agrees (`run`). WP-1117.
//@ expect: run
//@ strict-diags
//@ stdout: ~D2 ~D1 y3 ~D4 r5 ~D6 s7 ~D8 p9 ~D10 q11 | end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
D mk(int v) { return D{v}; }
struct P {
  int a;
};
struct L {
  char buf[32]{};
  int n = 0;
  constexpr void put(char c) { buf[n++] = c; }
};
struct E {
  L* l;
  char c;
  constexpr ~E() { l->put(c); }
};
constexpr L run() {
  L l;
  int y = ((void)E{&l, 'a'}, ((void)E{&l, 'b'}, 1));
  l.put(char('0' + y));
  P p = ((void)E{&l, 'c'}, P{2});
  l.put(char('0' + p.a));
  P&& q = ((void)E{&l, 'd'}.c, P{3});
  l.put(char('0' + q.a));
  return l;
}
static_assert(__builtin_strcmp(run().buf, "ba1c2d3") == 0);
int main() {
  int y = ((void)D{1}, (mk(2), 3));
  printf("y%d ", y);
  const int& r = ((void)mk(4), 5);
  printf("r%d ", r);
  static int s = (mk(6), 7);
  printf("s%d ", s);
  P p = ((void)D{8}, P{9});
  printf("p%d ", p.a);
  P&& q = (mk(10), P{11});
  printf("q%d | end\n", q.a);
}
