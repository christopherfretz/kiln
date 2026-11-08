// [basic.indet]: -Wno-sometimes-uninitialized drops only the
// 'whenever' warnings; -Wno-uninitialized-const-reference drops the const
// reference one; -Wconditional-uninitialized adds the 'may be' warning.
//@ expect: compile-ok
//@ args: -Wall -Wno-sometimes-uninitialized -Wno-uninitialized-const-reference -Wconditional-uninitialized
//@ strict-diags
//@ diag: 13:10 warning "variable 'x' is uninitialized when used here"
//@ diag: 12:8 note "initialize the variable 'x' to silence this warning"
//@ diag: 28:10 warning "variable 'z' may be uninitialized when used here"
//@ diag: 24:8 note "initialize the variable 'z' to silence this warning"
int always() {
  int x;
  return x;
}

int sometimes(bool b) {
  int y;
  if (b)
    y = 1;
  return y;
}

int maybe(bool b) {
  int z;
  for (int i = 0; i < 2; ++i)
    if (b)
      z = i;
  return z;
}

void take(const int &);
void const_ref() {
  int w;
  take(w);
}
