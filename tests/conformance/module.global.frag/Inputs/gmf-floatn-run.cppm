// gmf-floatn-limits-run.cpp: a GMF that includes <limits> before <math.h>
// (the importer includes them the other way round), whose purview
// reports numeric_limits<_Float32>/<_Float64> to the importer.
module;
#include <limits>
#include <math.h>
export module gfr;
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
export int gfr_digits32() { return std::numeric_limits<f32>::digits; }
export int gfr_digits64() { return std::numeric_limits<f64>::digits; }
export constexpr bool gfr_specialized = std::numeric_limits<f32>::is_specialized;
