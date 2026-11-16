// -fcheck-new (clang 22, GCC 16.1): every new-expression tests the
// pointer the allocation function returned against null before it
// initializes the object, even when that function is not declared
// noexcept ([expr.new]/20 requires the test only for a non-throwing
// allocation function); a null result yields a null new-expression
// and no initialization runs. clang and GCC test the reserved
// placement form too. Without an initializer and with a non-class
// type there is nothing to skip, so no test is emitted. Found running
// GCC's C++ testsuite.
//@ expect: run
//@ args: -fcheck-new
//@ strict-diags
//@ stdout: S null
//@ stdout: S[] null
//@ stdout: int null
//@ stdout: placement null
#include <cstdio>
#include <cstdlib>
#include <new>

void *none() { return nullptr; }

struct S {
  int v;
  S() : v(1) { std::puts("S constructed"); }
  ~S() { std::puts("S destroyed"); }
  static void *operator new(std::size_t) { return none(); }
  static void *operator new[](std::size_t) { return none(); }
  static void operator delete(void *) noexcept {}
  static void operator delete[](void *) noexcept {}
};

struct T {
  int v = 3;
};

int main() {
  S *p = new S;
  std::puts(p ? "S non-null" : "S null");
  S *a = new S[3];
  std::puts(a ? "S[] non-null" : "S[] null");
  int *q = new (static_cast<void *>(nullptr)) int(5);
  std::puts(q ? "int non-null" : "int null");
  T *t = new (static_cast<void *>(nullptr)) T;
  std::puts(t ? "placement non-null" : "placement null");
}
