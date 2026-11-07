// [expr.new]/28, [temp.deduct.decl]: when initialization of a
// placement-new object exits with an exception, the matching placement
// deallocation function may be a function template; its specialization is
// deduced against `void(void*, P...)` from the allocation function's
// parameter types. A template whose deduction fails or whose parameter
// count differs does not match (WP-713).
//@ std: c++20
//@ expect: run
//@ strict-diags
//@ stdout: 2 2
//@ stdout: C new 97
//@ stdout: C delete 97
//@ stdout: C new 7
#include <cstddef>
#include <cstdio>
#include <cstdlib>
struct Arena { char buf[64]; std::size_t used = 0; int news = 0, deletes = 0; };
template <class A> void* operator new(std::size_t n, A* a) {
  ++a->news;
  void* p = a->buf + a->used;
  a->used += n;
  return p;
}
template <class A> void operator delete(void*, A* a) noexcept { ++a->deletes; }
template <class A> void* operator new[](std::size_t n, A* a, int) { ++a->news; return std::malloc(n); }
template <class A> void operator delete[](void* p, A* a, int) noexcept { ++a->deletes; std::free(p); }
// A template whose deduction fails, and one with the wrong arity, do not match.
template <class A> void operator delete(void*, A*, A*) noexcept { std::puts("wrong"); }
int count = 0;
struct Throws { Throws() { if (count++ >= 0) throw 1; } };
struct C {
  int v;
  C(bool t) : v(1) { if (t) throw 2; }
  template <class T> static void* operator new(std::size_t n, T t) { std::printf("C new %d\n", int(t)); return std::malloc(n); }
  template <class T> static void operator delete(void* p, T t) { std::printf("C delete %d\n", int(t)); std::free(p); }
  static void operator delete(void* p) { std::free(p); }
};
int main() {
  Arena a;
  try { new (&a) Throws; } catch (int) {}
  try { new (&a, 4) Throws[2]; } catch (int) {}
  std::printf("%d %d\n", a.news, a.deletes);
  try { new ('a') C(true); } catch (int) {}
  delete new (7) C(false);
}
