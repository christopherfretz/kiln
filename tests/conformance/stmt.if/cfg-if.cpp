// [stmt.if]/1, [expr.log.and]/1, [expr.cond]/1: the control-flow graph of
// selection statements and of the short-circuit and conditional operators
// (spec/sema.md §7.5.4); a constant condition prunes the other branch.
//@ expect: cfg
void g();
int h(int);

int if_else(int x) {
  if (x)
    return 1;
  else
    g();
  return 2;
}

void if_init(int x) {
  if (int y = x; y > 1)
    g();
}

void if_true() {
  if (true)
    h(1);
  else
    h(2);
}

void logical(int x, int y) {
  if (x && y)
    g();
  if (x || h(y))
    g();
}

int conditional(int x) {
  int y = x ? h(1) : h(2);
  return y;
}

int nested(int x) {
  if (x > 0) {
    if (x > 10)
      return 2;
    return 1;
  }
  return 0;
}
