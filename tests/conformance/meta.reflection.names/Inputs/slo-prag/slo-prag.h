// Reached through `-I <dir>/Inputs/./slo-prag/` by
// source-location-of-include-path.cpp: a user header that makes itself a
// system header, so the directory a header included relative to it is
// looked up in becomes a system one.
#pragma GCC system_header
struct InPrag {};
constexpr const char* InPragFile = __builtin_FILE();
#include "../slo-prag-rel.h"
