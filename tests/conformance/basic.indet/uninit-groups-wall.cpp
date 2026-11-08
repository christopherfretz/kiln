// [basic.indet]: -Wall enables -Wuninitialized, which contains
// -Wsometimes-uninitialized and -Wuninitialized-const-reference;
// -Wconditional-uninitialized is off by default.
//@ expect: compile-ok
//@ args: -Wall
//@ strict-diags
//@ diag: 16:10 warning "variable 'x' is uninitialized when used here"
//@ diag: 15:8 note "initialize the variable 'x' to silence this warning"
//@ diag: 21:7 warning "variable 'y' is used uninitialized whenever 'if' condition is false"
//@ diag: 23:10 note "uninitialized use occurs here"
//@ diag: 21:3 note "remove the 'if' if its condition is always true"
//@ diag: 20:8 note "initialize the variable 'y' to silence this warning"
//@ diag: 37:8 warning "variable 'w' is uninitialized when passed as a const reference argument here"
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
