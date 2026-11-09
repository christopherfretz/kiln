// [module.global.frag]/1, [basic.def.odr]/15: the importer includes <limits>
// and imports a module whose GMF reached <limits> through <math.h> (as
// {fmt}'s module does through <cmath>). libstdc++'s explicit
// specializations numeric_limits<_Float32>/<_Float64>, emitted when the
// compiler predefines __FLT32_DIG__ and __FLT64_DIG__ (kiln does, clang
// does not), are one entity each: no redefinition. Found building the
// fmt_module OSS row against libstdc++ (docs/status/oss/fmt_module.md).
// The two units' specialization tokens differ (glibc's <math.h> defines
// `__builtin_nansf32` as a macro first), so they merge as entities, not by
// position (spec/modules.md §6, WP-1156).
//@ expect: compile-ok
//@ std: c++20
//@ module: gfm=Inputs/gmf-floatn-math.cppm
#include <limits>
import gfm;
static_assert(gfm_get() == 1);
static_assert(std::numeric_limits<float>::digits == 24);
#ifdef __FLT32_DIG__
static_assert(std::numeric_limits<_Float32>::digits == 24);
#endif
#ifdef __FLT64_DIG__
static_assert(std::numeric_limits<_Float64>::digits == 53);
#endif
int main() {}
