// [stmt.switch]/1, [stmt.goto]/1, [stmt.label]/1: the control-flow graph
// of switch statements and jumps (spec/sema.md §7.5.1).
//@ expect: cfg
void g();

void sw(int x) {
  switch (x) {
  case 1:
    g();
  case 2:
    break;
  default:
    g();
  }
  g();
}

void sw_no_default(int x) {
  switch (x) {
  case 1:
    g();
    break;
  case 2:
    return;
  }
}

enum E { A, B };
int sw_enum(E e) {
  switch (e) {
  case A:
    return 1;
  case B:
    return 2;
  }
  return 0;
}

void sw_const() {
  switch (2) {
  case 1:
    g();
    break;
  case 2:
    break;
  }
}

void jumps(int x) {
l:
  g();
  if (x)
    goto l;
}
