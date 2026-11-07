// [dcl.init.ref]/5.4.2: a `const S *const &` (or `S *const &`) bound to a
// `D *` binds a temporary holding the derived-to-base converted pointer.
// With a polymorphic second base `S` is not at offset 0, so the conversion
// must adjust the pointer (and keep a null pointer null). The retyped cast
// used to lose its base path (WP-782, LLVM's SCEVUnknown::deleted).
//@ expect: run
//@ stdout: 1 1 1 1 111 1 1
#include <cstdio>

struct V {
  virtual ~V() {}
  long v = 2;
};
struct S {
  long s = 1;
};
struct D : V, S {
  long d = 3;
};

long via_cref(const S *const &p) { return p ? p->s : -1; }
long via_ref(S *const &p) { return p ? p->s : -1; }
long via_ptr(const S *p) { return p ? p->s : -1; }

struct W : V, S {
  long g() { return via_cref(this) * 100 + via_ref(this) * 10 + via_ptr(this); }
};

int main() {
  D d;
  const S *const &r = &d;
  W w;
  D *null = nullptr;
  const S *const &rn = null;
  S *const &rn2 = null;
  std::printf("%ld %ld %ld %ld %ld %d %d\n", via_cref(&d), via_ref(&d),
              via_ptr(&d), r->s, w.g(), rn == nullptr, rn2 == nullptr);
}
