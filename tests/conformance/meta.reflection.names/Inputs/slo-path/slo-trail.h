// Reached through `-I <dir>/Inputs/slo-path//` (a doubled trailing slash)
// by source-location-of-include-path.cpp; includes a header relative to
// itself through `..`.
struct InTrail {};
constexpr const char* InTrailFile = __builtin_FILE();
#include "../slo-rel.h"
