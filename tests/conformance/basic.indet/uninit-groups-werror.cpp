// [basic.indet]: -Werror=uninitialized makes the group's warnings errors,
// -Wsometimes-uninitialized and -Wuninitialized-const-reference included.
//@ expect: compile-fail
//@ args: -Wall -Werror=uninitialized
//@ strict-diags
//@ diag: 15:10 error "variable 'x' is uninitialized when used here"
//@ diag: 14:8 note "initialize the variable 'x' to silence this warning"
//@ diag: 20:7 error "variable 'y' is used uninitialized whenever 'if' condition is false"
//@ diag: 22:10 note "uninitialized use occurs here"
//@ diag: 20:3 note "remove the 'if' if its condition is always true"
//@ diag: 19:8 note "initialize the variable 'y' to silence this warning"
//@ diag: 36:8 error "variable 'w' is uninitialized when passed as a const reference argument here"
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
