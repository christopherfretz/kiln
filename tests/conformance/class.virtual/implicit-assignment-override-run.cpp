// [class.virtual]/2, [class.copy.assign]/6: an implicitly declared copy or move
// assignment operator overrides a base's virtual assignment operator with the
// same parameter type, so a call through the base dispatches to it. kiln did
// not record the override, so the base's function ran instead (a
// miscompile; found and fixed by WP-957).
//@ expect: run
//@ stdout: 11 7
extern "C" int printf(const char *, ...);
struct D;
int calls = 0;
struct B {
  int x = 0;
  virtual D &operator=(const D &) { calls += 100; return *(D *)this; }
  virtual D &operator=(D &&) { calls += 1000; return *(D *)this; }
  virtual ~B() = default;
};
struct M {
  int v = 0;
  M &operator=(const M &o) { v = o.v; calls += 1; return *this; }
  M &operator=(M &&o) { v = o.v; calls += 10; return *this; }
};
struct D : B { M m; };
int main() {
  D a, b;
  b.m.v = 7;
  B &r = a;
  r = b;
  r = static_cast<D &&>(b);
  printf("%d %d\n", calls, a.m.v);
  return calls == 11 && a.m.v == 7 ? 0 : 1;
}
