// [dcl.fct.default]/5, [temp.inst]/5, [basic.def.odr]/8: a default
// argument is potentially evaluated only where a call uses it, so the
// functions its conversion names (`P<T>::P(nullptr_t)`, the temporary's
// `~P<T>`) are odr-used, and implicitly instantiated, only by such a
// call. `Never` is never completed, and `Decl`'s default argument is
// never used: `~P<Never>`, which needs a complete class, is not
// instantiated. The other default arguments are used after their types
// are completed, and the destructors run then. LLVM's
// `PGOIndirectCallPromotion(…, IntrusiveRefCntPtr<vfs::FileSystem> FS =
// nullptr)` with `FileSystem` incomplete (PGOInstrumentation.h).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 4 1|5|~3|D 1|~4|D 1|~4|D 1|~4|3
#include <cstdio>

struct FS;
template <class T> struct P {
  T *p = nullptr;
  P(decltype(nullptr)) {}
  P(const P &o) : p(o.p) {
    if (p)
      p->retain();
  }
  ~P() {
    if (p)
      p->release();
  }
};
struct Never;
struct Decl {
  Decl(int a = 0, P<Never> = nullptr);
};
struct Use {
  int a;
  Use(int a = 0, P<FS> = nullptr) : a(a) {}
};
struct Late {
  static void f(int x, P<FS> fs = {nullptr});
};
struct FS {
  int n = 0;
  void retain() { ++n; }
  void release() { --n; }
};
void Late::f(int x, P<FS> fs) { std::printf("%d %d|", x, fs.p == nullptr); }

template <class T> struct Q {
  int v;
  Q(int v) : v(v) {}
  ~Q() { std::printf("~%d|", static_cast<int>(sizeof(T)) + v); }
};
struct Inc;
void g(int a, Q<Inc> q = 0);
struct D {
  D(Q<Inc> q = 1) { std::printf("D %d|", q.v); }
};
struct Inc {
  char c[3];
};
void g(int a, Q<Inc>) { std::printf("%d|", a); }

int main() {
  Use u(3, nullptr);
  Late::f(4);
  g(5);
  D d;
  D arr[1];
  delete new D;
  std::printf("%d\n", u.a);
}
