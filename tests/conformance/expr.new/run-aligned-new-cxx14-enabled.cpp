// C++14 with -faligned-new (aligned allocation on, as in clang 22 and
// GCC 16.1) and -fnew-alignment=8 (clang's spelling of the threshold):
// the same overload choices as run-aligned-new-threshold.cpp at C++17.
// Found running GCC's C++ testsuite.
//@ expect: run
//@ std: c++14
//@ args: -faligned-new -fnew-alignment=8
//@ strict-diags
//@ stdout: new 4 plain
//@ stdout: delete plain
//@ stdout: new 8 plain
//@ stdout: delete plain
//@ stdout: new 16 aligned 16
//@ stdout: delete aligned 16
//@ stdout: new[] aligned 16
//@ stdout: delete[] aligned 16
//@ stdout: new[] plain
//@ stdout: delete[] plain
//@ stdout: 8
#include <cstdio>
#include <cstdlib>
#include <stdlib.h>
#include <new>

void *operator new(std::size_t n) {
  std::printf("new %zu plain\n", n);
  return std::malloc(n);
}
void *operator new(std::size_t n, std::align_val_t a) {
  std::printf("new %zu aligned %zu\n", n, static_cast<std::size_t>(a));
  return ::aligned_alloc(static_cast<std::size_t>(a), n);
}
void *operator new[](std::size_t n) {
  std::printf("new[] plain\n");
  return std::malloc(n);
}
void *operator new[](std::size_t n, std::align_val_t a) {
  std::printf("new[] aligned %zu\n", static_cast<std::size_t>(a));
  return ::aligned_alloc(static_cast<std::size_t>(a), (n + 15) / 16 * 16);
}
void operator delete(void *p) noexcept { std::puts("delete plain"); std::free(p); }
void operator delete(void *p, std::size_t) noexcept { std::puts("delete plain"); std::free(p); }
void operator delete(void *p, std::align_val_t a) noexcept {
  std::printf("delete aligned %zu\n", static_cast<std::size_t>(a));
  std::free(p);
}
void operator delete(void *p, std::size_t, std::align_val_t a) noexcept {
  std::printf("delete aligned %zu\n", static_cast<std::size_t>(a));
  std::free(p);
}
void operator delete[](void *p) noexcept { std::puts("delete[] plain"); std::free(p); }
void operator delete[](void *p, std::size_t) noexcept { std::puts("delete[] plain"); std::free(p); }
void operator delete[](void *p, std::align_val_t a) noexcept {
  std::printf("delete[] aligned %zu\n", static_cast<std::size_t>(a));
  std::free(p);
}
void operator delete[](void *p, std::size_t, std::align_val_t a) noexcept {
  std::printf("delete[] aligned %zu\n", static_cast<std::size_t>(a));
  std::free(p);
}

struct alignas(4) A4 { char c[4]; };
struct alignas(8) A8 { char c[8]; };
struct alignas(16) A16 { char c[16]; };

int main() {
  delete new A4;
  delete new A8;
  delete new A16;
  delete[] new A16[2];
  delete[] new A8[2];
  std::printf("%zu\n", static_cast<std::size_t>(__STDCPP_DEFAULT_NEW_ALIGNMENT__));
}
