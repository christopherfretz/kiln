// [stmt.goto] (GNU extension, computed goto): `&&label` is a
// `blockaddress`, also in a static initializer (a constant global); every
// indirect `goto *p` adds `p` to the `phi` of the function's one
// `indirectgoto` block, built on the first label address and placed after
// the return block, whose `indirectbr` lists each address-taken label once
// per `&&` (clang `GetAddrOfLabel` order, duplicates kept). With no
// `goto *` the `phi` becomes `poison`. `goto *&&l` is a plain `goto l`
// (through cleanups), and a conditional of two label addresses folds to a
// `select` (clang `EmitIndirectGotoStmt`, `GetIndirectGotoBlock`).
struct D {
  ~D();
};

int run(const unsigned char *code) {
  static void *t[] = {&&inc, &&dec, &&halt};
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

void twice(int c) {
  void *z;
  void *x = &&a;
  void *y = &&a;
  if (c)
    goto *x;
a:;
  z = &&b;
b:;
  (void)y;
  (void)z;
}

void no_goto() {
  void *x = &&a;
a:;
  (void)x;
}

void constant(int c) {
  {
    D d;
    if (c)
      goto *&&out;
  }
out:;
}

template <class T> T table(int i) {
  void *t[] = {&&x, &&y};
  goto *t[i];
x:
  return T(1);
y:
  return T(2);
}

int use() { return table<int>(0) + int(table<long>(1)); }
