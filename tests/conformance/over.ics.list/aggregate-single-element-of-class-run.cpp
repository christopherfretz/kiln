// [over.ics.list]/3 (CWG2076): a braced argument `{x}` whose only element is
// of aggregate class X (or a class derived from it) converts as `x` itself,
// not by aggregate initialization of X's members from `x`; for a reference
// parameter whose referee is reference-related to `x` the reference binds
// `x` ([dcl.init.list]/3.10), so `p({x})` with `x` an lvalue prefers
// `p(const O&)` over `p(O&&)`. rocksdb's `GetLiveFilesStorageInfo({opts},
// &files)` (spec/landed/sema/WP-724.md). WP-724.
//@ expect: run
//@ strict-diags
//@ stdout: ok 1 2 4 1 1 1 2 1 2 1 2 1 1
extern "C" int printf(const char *, ...);

struct O { bool a = false; unsigned long long b = 0; };
struct D : O {};
struct N { N() = default; N(const N &) = default; int v = 3; };
int f(const O &o) { return o.b == 7 ? 1 : 0; }
int g(O o) { return o.b == 7 ? 2 : 0; }
int p(const O &) { return 1; }
int p(O &&) { return 2; }
int q(O) { return 1; }
int q(long) { return 2; }
int s(const O &) { return 1; }
int s(const D &) { return 2; }
int t(O &) { return 1; }
int t(const volatile O &) { return 2; }
int u(N) { return 1; }
int u(long) { return 2; }
int same(const O &o, const O *x) { return &o == x; }
struct B { virtual int h(const O &o) { return o.a ? 4 : 0; } };

int main() {
  O x;
  x.b = 7;
  x.a = true;
  const O cx = x;
  D d;
  d.b = 7;
  N n;
  B bb;
  B *pb = &bb;
  printf("ok %d %d %d %d %d %d %d %d %d %d %d %d %d\n", f({x}), g({x}), pb->h({x}), f({cx}),
         f({d}), p({x}), p({O{}}), q({x}), s({d}), t({x}), t({cx}), u({n}), same({x}, &x));
  return 0;
}
