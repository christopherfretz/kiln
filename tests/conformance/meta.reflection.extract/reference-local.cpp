// [meta.reflection.extract] (extract-ref, extract-value): a block-scope
// reference whose lifetime began within the evaluation ([basic.life]/1)
// extracts to the object it refers to, as a value or as a reference
// (const included), in each block, loop iteration and recursive
// activation that declares it, and when it extends the lifetime of a
// temporary ([class.temporary]/6). GCC 16.1 rejects it, so no GCC oracle
// (spec/reflection-queries.md §5). Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
#include <meta>

consteval int by_local() {
  int x = 5;
  int &q = x;
  std::meta::extract<int &>(^^q) = 6;
  return x;
}
static_assert(by_local() == 6);

consteval int by_value() {
  int x = 5;
  const int &q = x;
  return std::meta::extract<int>(^^q) + std::meta::extract<const int &>(^^q);
}
static_assert(by_value() == 10);

consteval int loop() {
  int a[3] = {1, 2, 3}, sum = 0;
  for (int i = 0; i < 3; ++i) {
    int &e = a[i];
    sum += std::meta::extract<int>(^^e);
  }
  return sum;
}
static_assert(loop() == 6);

consteval int nested(int n) {
  int x = n;
  int &r = x;
  if (n > 0)
    std::meta::extract<int &>(^^r) += nested(n - 1);
  return std::meta::extract<int>(^^r);
}
static_assert(nested(3) == 6);

consteval int temporary() {
  const int &t = 40 + 2;
  int &&u = 1;
  return std::meta::extract<int>(^^t) + std::meta::extract<int>(^^u);
}
static_assert(temporary() == 43);

consteval int condition(int v) {
  int x = v;
  if (int &c = x; c > 0)
    return std::meta::extract<int>(^^c);
  return -1;
}
static_assert(condition(9) == 9);
