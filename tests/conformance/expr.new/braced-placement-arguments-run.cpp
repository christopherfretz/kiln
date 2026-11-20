// [expr.new]/1, /19: the new-placement is an expression-list, whose
// elements may be braced-init-lists ([expr.post.general]); overload
// resolution for the allocation function treats a braced placement
// argument as an initializer list ([over.ics.list]) and the parameter is
// copy-list-initialized from it, also in a template.
//@ expect: run
//@ stdout: 3 4 5 6 331
#include <cstdio>
#include <cstdlib>
struct P { int a, b; };
int seen;
void* operator new(decltype(sizeof 0) n, P p) { seen += p.a * 10 + p.b; return std::malloc(n); }
void* operator new(decltype(sizeof 0) n, int k, P p) { seen += k * 100 + p.a + p.b; return std::malloc(n); }
template<class T> T* make(T v) { return new (2, {v, v}) T(v); }
int main() {
  int* a = new ({1, 2}) int(3);
  int* b = new (1, {3, 4}) int(4);
  int* c = new (0, {}) int(5);
  int* d = make(6);
  std::printf("%d %d %d %d %d\n", *a, *b, *c, *d, seen);
  std::free(a); std::free(b); std::free(c); std::free(d);
}
