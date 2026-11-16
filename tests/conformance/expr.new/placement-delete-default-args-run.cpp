// [expr.new]/24,28: `new A` may call a placement allocation function whose
// parameters after the size all have default arguments; if the
// initialization throws, the matching placement deallocation function is
// called with the same arguments (clang passes the default arguments).
// Found running GCC's C++ testsuite (WP-1467).
//@ expect: run
//@ stdout: new 1.5
//@ stdout: delete 1.5
//@ stdout: caught
//@ stdout: new 1.5
//@ stdout: delete 1.5
//@ stdout: caught
#include <cstddef>
#include <cstdio>
#include <new>
struct A {
  // Called by `new A`: the placement form with a default argument.
  void *operator new(std::size_t n, float f = 1.5f) {
    std::printf("new %.1f\n", f);
    return ::operator new(n);
  }
  void operator delete(void *p) { ::operator delete(p); }
  // The matching deallocation function gets the same argument.
  void operator delete(void *p, float f) {
    std::printf("delete %.1f\n", f);
    ::operator delete(p);
  }
  A() { throw 5; }
};
template <class T> T *make() { return new T; }
int main() {
  try { (void)new A; } catch (int) { std::puts("caught"); }
  try { (void)make<A>(); } catch (int) { std::puts("caught"); }
}
