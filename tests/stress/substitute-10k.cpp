// [meta.reflection.substitute] (P2996R13), spec/reflection.md §3.3: 10,000
// distinct substitutions (Box^10000<int>, each forming a specialization
// that is never instantiated) and then the same 10,000 again, which the
// (template, arguments) memo answers. The constexpr heap is untouched
// (the arguments are braced lists) and the run stays small.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ oracle: gcc
//@ strict-diags
//@ oven: peak_bytes<=64 allocs<=0 steps<=40000
//@ mem<=1024
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;

template <class T> struct Box {};

consteval info boxes(int n) {
  info r = ^^int;
  for (int k = 0; k < n; ++k) r = substitute(^^Box, {r});
  return r;
}

constexpr info b = boxes(10000);
static_assert(b == boxes(10000));  // memo hits
static_assert(template_of(b) == ^^Box);
static_assert(can_substitute(^^Box, {b}));

int main() {}
