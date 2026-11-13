// [class.temporary]/4-6 (a temporary is destroyed at the end of its
// full-expression unless bound to a reference, which extends it to the
// reference's lifetime), [except.ctor]/1-2 (automatic objects are
// destroyed when a throw leaves their scope): with the flags of
// a downstream release build (`-O2 -g -fPIC`; its `-march`
// is irrelevant here and omitted so the test runs on every host) the
// lifetime markers of materialized temporaries,
// lifetime-extended temporaries and of variables whose scope a throw
// leaves (the llvm.lifetime.end in the EH cleanup) must use the
// signature of the backend's LLVM: kiln emitted LLVM 22's sizeless
// `llvm.lifetime.start.p0(ptr)` for an LLVM 21 `opt`, which rejected the
// module. The shapes are that build's: a guard object with a destructor around
// a call that may throw, `const T&` bound to a returned aggregate, and a
// temporary argument.
//@ expect: run
//@ args: -O2 -g -fPIC
//@ stdout: d2 d1 caught 3 d4 24 d6 d5 13
extern "C" int printf(const char*, ...);

struct Guard {
  int id;
  long pad[3];
  ~Guard() { printf("d%d ", id); }
};
struct Agg { int v[6]; };

[[gnu::noinline]] void may_throw(int n) { if (n > 2) throw n; }
[[gnu::noinline]] Agg make(int n) { return Agg{{n, n, n, n, n, n + 1}}; }
[[gnu::noinline]] int total(const Agg& a) {
  int s = 0;
  for (int e : a.v) s += e;
  return s;
}

int guarded(int n) {
  Guard g1{1, {}};
  {
    Guard g2{2, {}};
    Agg local = make(n);
    may_throw(total(local) / 6);
  }
  return 0;
}

int extended(int n) {
  const Agg& r = make(n);   // lifetime-extended to the end of the block
  Guard g4{4, {}};
  return total(r) + total(make(n - 4)) - 2;  // and a full-expression temporary
}

int main(int argc, char**) {
  try {
    guarded(argc + 2);
  } catch (int e) {
    printf("caught %d ", e);
  }
  printf("%d ", extended(argc + 3));
  int t = total(Agg{{argc, 2, 3, 4, 0, 0}}) + [&] {
    Guard g5{5, {}};
    Guard g6{6, {}};
    return argc + 2;
  }();
  printf("%d\n", t);
}
