// [expr.new]/7 (C++03): before C++11 an invalid run-time array bound is undefined behaviour and
// std::bad_array_new_length does not exist; kiln passes SIZE_MAX to the allocation function, as
// clang does, so it throws std::bad_alloc. Found running GCC's C++ testsuite.
//@ expect: run
//@ args: -std=c++98
//@ stdout: ok
#include <cstdio>
#include <cstdlib>
#include <new>
struct K {
  static std::size_t seen;
  static void* operator new[](std::size_t n) { seen = n; throw std::bad_alloc(); }
  static void operator delete[](void*) {}
  int k;
};
std::size_t K::seen;
int main(int argc, char**) {
  volatile int neg = -argc;
  try { (void)new K[neg]; } catch (const std::bad_alloc&) {}
  std::puts(K::seen == ~std::size_t(0) ? "ok" : "wrong");
}
