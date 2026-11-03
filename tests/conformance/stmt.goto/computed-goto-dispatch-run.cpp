// [stmt.goto] (GNU computed goto): `goto *p` jumps to the label whose
// address `p` holds, taken with `&&label` in the same function: from a
// static table (a constant initializer), a local table, a conditional and
// a variable; every address-taken label is reachable, `&&l` taken twice is
// one address, and `goto *&&l` is `goto l` (it runs the cleanups of the
// scopes it leaves). Each specialization of a template jumps to its own
// labels.
//@ expect: run
//@ stdout: acc=2
//@ stdout: p0=1 p1=2
//@ stdout: ~D,out
//@ stdout: t=3
//@ stdout: same=1
extern "C" int printf(const char *, ...);

int run(const unsigned char *code) {
  static void *const t[] = {&&inc, &&dec, &&halt};
  int acc = 0;
  goto *t[*code++];
inc:
  ++acc;
  goto *t[*code++];
dec:
  --acc;
  goto *t[*code++];
halt:
  return acc;
}

int pick(bool c) {
  goto *(c ? &&a : &&b);
a:
  return 1;
b:
  return 2;
}

struct D {
  ~D() { printf("~D,"); }
};

void leave(int c) {
  {
    D d;
    if (c)
      goto *&&out;
    printf("not reached,");
  }
out:
  printf("out\n");
}

template <class T> T table(int i) {
  void *t[] = {&&x, &&y};
  goto *t[i];
x:
  return T(1);
y:
  return T(2);
}

bool same() {
  void *p = &&l;
  void *q = &&l;
l:
  return p == q;
}

int main() {
  const unsigned char prog[] = {0, 0, 1, 0, 1, 0, 2};
  printf("acc=%d\n", run(prog));
  printf("p0=%d p1=%d\n", pick(true), pick(false));
  leave(1);
  printf("t=%d\n", table<int>(0) + int(table<long>(1)));
  printf("same=%d\n", same());
  return 0;
}
