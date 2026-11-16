// -fno-check-new (the default) after -fcheck-new: the last flag wins,
// so a throwing class operator new that returns a non-null pointer is
// used without a null test and the object is initialized as usual
// ([expr.new]/20). Found running GCC's C++ testsuite.
//@ expect: run
//@ args: -fcheck-new -fno-check-new
//@ strict-diags
//@ stdout: constructed 7
#include <cstdio>
#include <cstdlib>
#include <new>

struct S {
  int v;
  S() : v(7) { std::printf("constructed %d\n", v); }
  static void *operator new(std::size_t n) { return std::malloc(n); }
  static void operator delete(void *p) noexcept { std::free(p); }
};

int main() { delete new S; }
