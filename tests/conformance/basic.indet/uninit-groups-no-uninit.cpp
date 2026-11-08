// [basic.indet]: -Wno-uninitialized silences the whole group,
// -Wsometimes-uninitialized and -Wuninitialized-const-reference included.
//@ expect: compile-ok
//@ args: -Wall -Wno-uninitialized
//@ strict-diags
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
