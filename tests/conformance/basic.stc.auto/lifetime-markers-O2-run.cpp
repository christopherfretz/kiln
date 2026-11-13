// [basic.stc.auto]/1 (the storage of a block variable lasts until its
// block exits), [stmt.dcl]/2-3 (a jump past a declaration), [stmt.jump]/2:
// above -O0 every automatic variable's alloca is bracketed by
// llvm.lifetime.start/.end (clang ShouldEmitLifetimeMarkers), and the
// optimizer may then share stack slots between variables whose lifetimes
// do not overlap. The intrinsics' signature depends on the LLVM that
// consumes the IR: LLVM 19 and 21 take `(i64 size, ptr)` and their
// verifiers reject the sizeless form, LLVM 22 takes only `(ptr)`. kiln
// emitted LLVM 22's form for an LLVM 21 backend at -fclang-compat=22, so
// every -O1+ compile of a function with a local aggregate failed in `opt`
// ("Intrinsic has incorrect argument type! llvm.lifetime.start.p0"). The
// shape is a downstream RelWithDebInfo tarball build: block-scope
// aggregates passed by address to out-of-line functions, in nested
// blocks, loops, `switch`es and behind `goto`s.
//@ expect: run
//@ args: -O2
//@ stdout: 8 18 6 4 2 400 2
extern "C" int printf(const char*, ...);

struct A { int x[4]; };
struct B { long y[8]; };

[[gnu::noinline]] void fill(A* a, int v) { for (int& e : a->x) e = v; }
[[gnu::noinline]] int sum(const A* a) { return a->x[0] + a->x[1] + a->x[2] + a->x[3]; }
[[gnu::noinline]] void fillb(B* b, long v) { for (long& e : b->y) e = v; }
[[gnu::noinline]] long sumb(const B* b) { long s = 0; for (long e : b->y) s += e; return s; }

// Two sibling blocks: the slots may be shared, the values must not mix.
int siblings(int n) {
  int r = 0;
  {
    A a;
    fill(&a, n);
    r += sum(&a);
  }
  {
    A b;
    fill(&b, n / 2);
    r += sum(&b) - n * 2;
  }
  return r;
}

// A loop body's variable starts and ends its lifetime every iteration.
long loop(int n) {
  long r = 0;
  for (int i = 0; i < n; ++i) {
    B b;
    fillb(&b, i);
    r += sumb(&b) / 8;
    if (i == 2) continue;
    A a;
    fill(&a, 1);
    r += sum(&a);
  }
  return r;
}

// Variables a `switch` or `goto` jumps past get no markers.
int jumps(int k) {
  int r = 0;
  switch (k) {
    A a;
  case 1:
    fill(&a, 1);
    r += sum(&a);
    [[fallthrough]];
  case 2: {
    A b;
    fill(&b, 2);
    r += sum(&b) - 6;
  }
  }
  if (k > 0) goto skip;
  {
    A c;
    fill(&c, 100);
    r += sum(&c);
  }
skip:
  return r;
}

// An array of aggregates and an aggregate temporary.
int arrays() {
  A as[3];
  for (int i = 0; i < 3; ++i) fill(&as[i], i + 1);
  int r = sum(&as[0]) + sum(&as[1]) + sum(&as[2]);
  return r - sum(&(const A&)A{{1, 2, 3, 4}}) * 2;
}

int main(int argc, char**) {
  printf("%d %ld %d %d %d %d %d\n", siblings(argc + 1), loop(argc + 3), jumps(argc), arrays(),
         jumps(argc + 1), jumps(argc - 1), siblings(argc));
}
