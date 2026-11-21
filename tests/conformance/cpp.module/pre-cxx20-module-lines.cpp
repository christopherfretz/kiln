// [cpp.pre]/1, [cpp.module], [cpp.import] (C++20): before C++20 a line
// starting with `module` or `import` is no directive; the names are
// ordinary identifiers, here typedef names declaring two variables, as
// clang 22 compiles it.
//@ std: c++17
//@ strict-diags
typedef int import;
typedef int module;
import m;
module x;
static_assert(sizeof(m) == sizeof(x), "");
