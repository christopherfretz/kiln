// [module.import]/5: `import <header>;` of an importable library header
// (here `<cstddef>`) makes its declarations and macros available.
//@ expect: run
//@ std: c++20
//@ skip(clang): clang needs -fmodule-header=system to build <cstddef> as a header unit first, which the harness cannot drive
import <cstddef>;

#ifndef offsetof
#error "offsetof comes from the header unit"
#endif

struct P {
  int a, b;
};

int main() {
  std::size_t n = offsetof(P, b);
  return n == sizeof(int) ? 0 : 1;
}
