// [meta.reflection.extract], [expr.const]/4: a reference variable that is
// constant-initialized is usable in constant expressions without
// constexpr, so extract<T&> of it yields the object it refers to. Found
// running GCC's C++ testsuite.
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

static const int j = 7;
const int &rj = j;
static_assert(extract<const int &>(^^rj) == 7);
static_assert(&extract<const int &>(^^rj) == &j);

int k = 1;
int &rk = k;
static_assert(&extract<int &>(^^rk) == &k);

struct P { int x, y; };
constexpr P p{1, 2};
const P &rp = p;
static_assert(extract<const P &>(^^rp).y == 2);
