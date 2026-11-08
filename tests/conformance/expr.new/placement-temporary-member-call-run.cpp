// [expr.new]/1, [dcl.ambig.res]/2: in `new (A<T>().allocate(1)) T(x)` the parenthesized
// tokens start like the type-id `A<T>()` (a function type) but continue with `.`, so they are
// a new-placement whose argument calls a member of a temporary (oneTBB's private_server).
//@ expect: run
//@ stdout: 5 7
#include <cstdio>
#include <new>
template <class T> struct A {
  T* allocate(int n) {
    alignas(T) static unsigned char b[4 * sizeof(T)];
    return reinterpret_cast<T*>(b) + n;  // distinct slots
  }
};
struct S { int v; S(int x) : v(x) {} };
int main() {
  S* p = new (A<S>().allocate(1)) S(5);
  S* q = ::new (A<S>().allocate(2)) S{7};
  std::printf("%d %d\n", p->v, q->v);
}
