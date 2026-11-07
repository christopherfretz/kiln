// [expr.new]/20, [temp.over]/1: a global allocation function template is
// an allocation candidate. Its specialization is deduced from the size and
// the placement arguments and competes with the non-template functions
// (the library's `operator new(size_t, void*)` among them) in overload
// resolution: an exact-match template beats a conversion, and on a tie
// the non-template function wins ([over.match.best]/2.4) (WP-712).
//@ std: c++20
//@ expect: run
//@ strict-diags
//@ stdout: 5 2 1
//@ stdout: new[] 3
//@ stdout: 1 2 2
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <new>
struct Arena { char buf[64]; std::size_t used = 0; int news = 0, deletes = 0; };
template <class A> void* operator new(std::size_t n, A* a) {
  ++a->news;
  void* p = a->buf + a->used;
  a->used += n;
  return p;
}
template <class A> void operator delete(void*, A* a) noexcept { ++a->deletes; }
template <class Tag> void* operator new[](std::size_t n, Tag, int k) { std::printf("new[] %d\n", k); return std::malloc(n); }
template <class Tag> void operator delete[](void* p, Tag, int) noexcept { std::free(p); }
struct Pool { int n = 0; };
void* operator new(std::size_t n, Pool& p) { ++p.n; return std::malloc(n); }
template <class T> void* operator new(std::size_t n, T& t) { t.n += 100; return std::malloc(n); }
template <class T> void* operator new(std::size_t n, T& t, long) { ++t.n; return std::malloc(n); }
struct Throws { Throws() { throw 1; } };
struct V { int v; V(int x) : v(x) {} };
struct Tag {};
int main() {
  Arena a;
  V* v = new (&a) V(5);
  try { new (&a) Throws; } catch (int) {}
  std::printf("%d %d %d\n", v->v, a.news, a.deletes);
  int* p = new (Tag{}, 3) int[2];
  delete[] p;
  Pool pool;
  int* q = new (pool) int(1);
  int* r = new (pool, 2L) int(2);
  std::printf("%d %d %d\n", *q, *r, pool.n);
  std::free(q); std::free(r);
}
