// [dcl.asm] (GNU extension): `asm goto` lowers to `callbr` with one `!i`
// constraint per label after the inputs (and "+" input halves); `%l1` and
// `%l[name]` number the labels after the operands. With register outputs,
// every indirect edge gets its own block storing them; an `unwind` clobber
// inside a try block is an `invoke`.
void g();
int f(int x) {
  asm goto("# %l1" : : "r"(x) : "memory" : zero);
  return 0;
zero:
  return 1;
}
int g(int x) {
  int y, z;
  asm goto("" : "=r"(y), "=r"(z) : "r"(x) : : a, b);
  return y;
a:
  return z;
b:
  return 3;
}
int h(int x) {
  int y;
  asm goto("" : "+r"(y) : : : out);
  return y;
out:
  return -y;
}
int k(int x) {
  asm goto("# %l[l2]" : : : : l1, l2);
l1:
  return 1;
l2:
  return 2;
}
int loop(int x) {
  int s = 0;
  for (int i = 0; i < x; ++i) {
    int y;
    asm goto("" : "=r"(y) : "r"(i) : : skip, done);
    s += y;
    continue;
  skip:
    g();
  }
done:
  try {
    g();
    asm goto("" ::: "memory" : in);
  in:
    g();
  } catch (...) {
    return -1;
  }
  return s;
}
int two(bool b) {

  if (b) return 1;
  asm goto("" :::: l1, l2);
  return 2;
l2:
  g();
l1:
  return 3;
}
int unwind_clobber(int x) {
  int r = 0;
  try {
    asm volatile("" : "=r"(r) : "r"(x) : "unwind");
    asm("" ::: "unwind");
  } catch (...) {
    return -1;
  }
  return r;
}
int named_after_plus(int x) {
  int y = x;
  asm goto("# %l[out]" : "+r"(y) : : : skip, out);
  return y;
skip:
  return -1;
out:
  return y + 1;
}
