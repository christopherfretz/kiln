// [stmt.ranged]/1, [stmt.dcl]/3, [except.handle]/1, [stmt.while]/2: a
// range-based for, a local static's guarded initialization, nested try
// blocks and a condition variable (the control-flow graph, spec/sema.md
// §7.5.1).
//@ expect: cfg
//@ cfg-diverges: local_static clang's analyzer builds a `static init` branch (CFGBuildOptions::AddStaticInitBranches) that its warnings' graph has not
void g();
int h(int);

int sum(const int (&a)[4]) {
  int s = 0;
  for (int v : a)
    s += v;
  return s;
}

int local_static(int x) {
  static int k = h(x);
  return k;
}

void tries(int x) {
  try {
    try {
      if (x)
        throw 1;
      g();
    } catch (long) {
      throw;
    }
  } catch (int) {
    g();
  }
}

void cond_var(int x) {
  while (int y = h(x))
    x -= y;
}

void lambda(int x) {
  auto f = [x] { return h(x); };
  f();
}
