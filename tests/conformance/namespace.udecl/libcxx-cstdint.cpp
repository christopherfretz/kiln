// libc++'s <cstdint> is built from using-declarations with
// __attribute__((__using_if_exists__)) (WP-062): it must parse, and its
// names must resolve through the shadows.
//@ expect: compile-ok
//@ skip(clang): kiln-only regression check of libc++'s <cstdint>
//@ args: -stdlib=libc++
#include <cstdint>
std::uint32_t u = 1;
std::int64_t i = -1;
std::uintptr_t p = 0;
static_assert(sizeof(std::int8_t) == 1 && sizeof(std::uint64_t) == 8);
static_assert(INT32_MAX == 2147483647);
using std::int16_t;
int16_t s = 2;
int use() {
  using std::uint8_t;
  uint8_t b = 255;
  return b + s;
}
