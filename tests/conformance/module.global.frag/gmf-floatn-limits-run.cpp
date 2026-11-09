// [module.global.frag]/1, [basic.def.odr]/15, [numeric.limits.general]:
// the importer includes <math.h> then <limits> before importing a module
// whose GMF included them the other way round. With kiln's __FLT32_DIG__
// and __FLT64_DIG__, libstdc++'s numeric_limits<_Float32>/<_Float64>
// explicit specializations are defined in both units (in different token
// streams: glibc's <math.h> defines `__builtin_nansf32` as a macro); they
// are one entity each, and the module's functions and the importer agree
// on `digits` across the module boundary (clang, without the macros, has
// `_Float32` as glibc's typedef of float: the same values). WP-1156.
//@ expect: run
//@ std: c++20
//@ module: gfr=Inputs/gmf-floatn-run.cppm
//@ stdout: 24 24 53 53 1
#include <math.h>
#include <limits>
import gfr;
extern "C" int printf(const char *, ...);
#ifdef __FLT32_DIG__
using f32 = _Float32;
#else
using f32 = float;
#endif
#ifdef __FLT64_DIG__
using f64 = _Float64;
#else
using f64 = double;
#endif
static_assert(gfr_specialized && std::numeric_limits<f32>::is_specialized);
int main() {
  printf("%d %d %d %d %d\n", gfr_digits32(), std::numeric_limits<f32>::digits, gfr_digits64(),
         std::numeric_limits<f64>::digits, int(gfr_specialized));
}
