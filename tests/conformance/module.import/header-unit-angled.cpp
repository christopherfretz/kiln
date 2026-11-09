// [module.import]/5: `import <header>;` of an importable library header
// (here `<cstddef>`) makes its declarations and macros available. kiln
// builds the header unit in-process at the import; a clang builds it first
// with `-fmodule-header=system` (spec/harness.md `header-unit:`).
//@ expect: run
//@ std: c++20
//@ args: -Wno-experimental-header-units
//@ header-unit: <cstddef>
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
