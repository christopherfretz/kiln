// Reached through `-isystem <dir>//Inputs/slo-sys/../slo-sys/./` by
// source-location-of-include-path.cpp; includes a header relative to
// itself through `..`.
struct InSys {};
constexpr const char* InSysFile = __builtin_FILE();
#include "../slo-sys-rel.h"
