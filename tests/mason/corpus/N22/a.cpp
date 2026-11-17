#include "h.h"
// the strong out-of-line definition (C++14)
constexpr int A::n;
const int *keep_a = &A::n;
