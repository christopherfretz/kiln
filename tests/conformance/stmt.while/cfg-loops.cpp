// [stmt.while]/1, [stmt.do]/1, [stmt.for]/1, [stmt.ranged]/1,
// [stmt.break]/1, [stmt.cont]/1: the control-flow graph of iteration
// statements (spec/sema.md §7.5.4); a loop whose condition is a constant
// drops the dead edge (clang prints it as NULL).
//@ expect: cfg
void g();
int h(int);

void while_loop(int x) {
  while (x)
    x = h(x);
}

void while_true(int x) {
  while (true) {
    if (h(x))
      break;
  }
  g();
}

void do_loop(int x) {
  do
    g();
  while (x);
}

void do_false() {
  do
    g();
  while (false);
}

int for_loop(int x) {
  for (int i = 0; i < x; ++i) {
    if (i)
      continue;
    g();
  }
  return 0;
}

void for_ever() {
  for (;;)
    g();
}

void range_for() {
  int a[2] = {1, 2};
  for (int i : a)
    h(i);
}
