// Source-based coverage mapping (WP-1003, spec/codegen.md §5.4
// "Instrumentation"): under -fprofile-instr-generate -fcoverage-mapping
// every function gets a `__covrec_` record (code, gap, branch, skipped
// and expansion regions over the WP-941 counters) and the TU one
// `__llvm_coverage_mapping` header with its filenames; inline and
// template functions get `linkonce_odr` records and unused ones zero
// records named in `__llvm_coverage_names`. kiln writes the filenames
// uncompressed: the comparison asks clang for the same
// (`-mllvm -enable-name-compression=false`, added by ir_vs_clang), so the
// TU hash agrees. No `&&`/`||` or constant-folded condition: clang 20+
// maps those differently and kiln follows clang 19 (WP-1003 follow-up).
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
#define TWICE(x) ((x) + (x))
#if 0
int skipped() { return 0; }
#endif
int ext(int);

int branches(int x) {
  int r = 0;
  if (x > 0)
    r = 1;
  else if (x < -5)
    r = 2;
  else {
    r = 3;
  }
  return x ? r : TWICE(r);
}

int loops(int n) {
  int s = 0;
  for (int i = 0; i < n; ++i) {
    if (i == 3)
      continue;
    if (i == 7)
      break;
    s += i;
  }
  while (s > 100)
    s /= 2;
  do
    --s;
  while (s > 50);
  int a[] = {1, 2, 3};
  for (int v : a)
    s += v;
  return s;
}

int sw(int k) {
  switch (k) {
  case 0:
    return 1;
  case 1:
  case 2:
    k += 4;
    [[fallthrough]];
  default:
    break;
  }
  return k;
}

int guarded(int v) {
  try {
    return ext(v);
  } catch (int e) {
    return e;
  } catch (...) {
    throw;
  }
}

template <class T> T square(T v) { return v * v; }
inline int unused_inline(int x) { return x + 1; }
static int unused_static(int x) { return x - 1; }

struct Counter {
  int n = 0;
  Counter() = default;
  void bump(int by) {
    if (by > 0)
      n += by;
  }
};

int use() {
  auto f = [](int y) { return y > 2 ? y : -y; };
  Counter c;
  c.bump(f(3));
  return square(c.n) + square(2L);
}
