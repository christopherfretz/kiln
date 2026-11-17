// [expr.delete]/7 at run time: the deallocation function is called even
// when the destructor exits by an exception, for `delete` and `delete[]`
// (the remaining elements are destroyed first, [except.ctor]/2), through
// a virtual destructor (the deleting destructor), and for a class-specific
// `operator delete`; a destroying `operator delete` is called instead of
// the destructor ([expr.delete]/6), so the destructor never runs. Counted
// with replaced global deallocation functions.
//@ expect: run
//@ stdout: scalar 1 2
//@ stdout: array 1 4 order 4321
//@ stdout: virtual 2 7
//@ stdout: class 1 2 11
//@ stdout: destroying 1 1 11
#include <cstdio>
#include <cstdlib>
#include <new>
static int frees, arrfrees, classfrees, destroying;
void *operator new(std::size_t n) { return std::malloc(n ? n : 1); }
void *operator new[](std::size_t n) { return std::malloc(n ? n : 1); }
void operator delete(void *p) noexcept { ++frees; std::free(p); }
void operator delete(void *p, std::size_t) noexcept { ++frees; std::free(p); }
void operator delete[](void *p) noexcept { ++arrfrees; std::free(p); }
void operator delete[](void *p, std::size_t) noexcept { ++arrfrees; std::free(p); }
static int order;
struct T { int i; ~T() noexcept(false) { order = order * 10 + i; if (i == 2) throw i; } };
struct B { virtual ~B() noexcept(false) {} };
struct D : B { ~D() noexcept(false) override { throw 3; } };
struct C {
  ~C() noexcept(false) { throw 4; }
  static void operator delete(void *p) noexcept { ++classfrees; std::free(p); }
};
struct X {
  int k;
  ~X() noexcept(false) { throw 5; }
  void operator delete(X *p, std::destroying_delete_t) { ++destroying; ::operator delete(p); }
};
int main() {
  int caught = 0;
  try { delete new T{2}; } catch (int e) { caught += e; }
  std::printf("scalar %d %d\n", frees, caught);
  T *a = new T[4]{{1}, {2}, {3}, {4}};
  order = 0;
  try { delete[] a; } catch (int e) { caught += e; }
  std::printf("array %d %d order %d\n", arrfrees, caught, order);
  B *b = new D;
  try { delete b; } catch (int e) { caught += e; }
  std::printf("virtual %d %d\n", frees, caught);
  try { delete new C; } catch (int e) { caught += e; }
  std::printf("class %d %d %d\n", classfrees, frees, caught);
  int before = frees;
  delete new X{1};
  std::printf("destroying %d %d %d\n", destroying, frees - before, caught);
}
