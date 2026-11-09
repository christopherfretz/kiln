// gmf-floatn-limits.cpp: a GMF that includes <math.h> before <limits>
// (libstdc++'s <math.h> reaches <limits> through <cmath>). glibc's
// `typedef float _Float32;` comes first, then libstdc++'s
// numeric_limits<_FloatN> specializations.
module;
#include <math.h>
export module gfm;
export constexpr int gfm_get() { return 1; }
