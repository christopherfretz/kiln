// [cpp.module]/1, [module.global.frag]: macros defined in the global module
// fragment stay defined in the purview; the directive ends at its `;`.
//@ std: c++20
//@ strict-diags
module;
#define VALUE 3
export module m.dir;
export constexpr int v = VALUE;
static_assert(v == 3);
