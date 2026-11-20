// [basic.lookup.qual.general]/4.3, [expr.prim.id.dtor]/2: `~Q` after a
// member-qualified `F` on a dependent object (`p.T::~Q()`) is looked up
// as in `p.~Q()`, in the object's class at instantiation; a dependent
// qualifier on a class object (`s->T::~S()`, `~S` found there) is checked
// against the object then.
//@ expect: run
//@ stdout: ~M ~M ~S ~S |
#include <cstdio>
#include <new>
struct M { using Zq = M; ~M() { std::printf("~M "); } };
struct S { using Q = S; ~S() { std::printf("~S "); } };
template <class T> void a11(T &p) { p.T::~Zq(); }
template <class T> void a12(T *p) { p->T::~Zq(); }
template <class T> void g4(S *s) { s->T::~S(); }
// In a generic lambda of a template: the object is still dependent at
// the first instantiation.
template <class T> void gl(T *p) {
  auto l = [](auto *q) { q->T::~Q(); };
  l(p);
}
int main() {
  alignas(16) unsigned char b[64];
  a11(*new (b) M);
  a12(new (b) M);
  g4<S>(new (b) S);
  gl(new (b) S);
  std::printf("|\n");
}
