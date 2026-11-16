// -fno-aligned-new (GCC; clang 22's -fno-aligned-allocation) at C++17:
// no new-expression calls the std::align_val_t allocation functions,
// even for a type with new-extended alignment, and
// __cpp_aligned_new is not defined; __STDCPP_DEFAULT_NEW_ALIGNMENT__
// stays defined, as in clang (GCC leaves it undefined). The C++14
// default is the same (run-aligned-new-cxx14.cpp). Found running GCC's
// C++ testsuite.
//@ expect: run
//@ std: c++17
//@ args: -fno-aligned-new
//@ strict-diags
//@ stdout: new 64 plain
//@ stdout: delete plain
//@ stdout: no __cpp_aligned_new
#include <cstdio>
#include <cstdlib>
#include <new>

void *operator new(std::size_t n) {
  std::printf("new %zu plain\n", n);
  return std::malloc(n);
}
void operator delete(void *p) noexcept { std::puts("delete plain"); std::free(p); }
void operator delete(void *p, std::size_t) noexcept { std::puts("delete plain"); std::free(p); }

struct alignas(64) A64 { char c[64]; };

int main() {
  delete new A64;
#ifdef __cpp_aligned_new
  std::puts("__cpp_aligned_new");
#else
  std::puts("no __cpp_aligned_new");
#endif
}
