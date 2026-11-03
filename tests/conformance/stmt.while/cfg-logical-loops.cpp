// [stmt.while]/1, [stmt.do]/1, [stmt.for]/1, [expr.log.and]/1,
// [expr.log.or]/1: a loop condition built of `&&` and `||` branches
// straight to the body and the exit (the control-flow graph,
// spec/sema.md §7.5.1).
//@ expect: cfg
void g();
int h(int);

void w(int x, int y) {
  while (x && (y || h(x)))
    g();
}

void d(int x, int y) {
  do
    g();
  while ((x || y) && h(y));
}

void f(int x) {
  for (int i = 0; i < x && h(i); ++i)
    g();
}

void k(int x) {
  if ((x && h(1)) || (h(2) && x))
    g();
  else if (false && x)
    g();
}
