// [meta.reflection.extract] (LWG4483): extract<T*> of a variable of array
// type yields a pointer to its first element when T is
// qualification-compatible with the element type, so an array of arrays
// extracts as a pointer to its first row. Found running GCC's C++
// testsuite.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

constexpr int m[2][3]{{1, 2, 3}, {4, 5, 6}};
static_assert(extract<const int (*)[3]>(^^m)[1][2] == 6);
