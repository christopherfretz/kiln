// A unit of ../inline-member-two-compilers-kiln-first-run.cpp compiled by
// the compiler under test, linked ahead of the clang unit.
#include "inline_member_two_compilers.h"
Units from_kiln() { return {&A::n, &A::m, &A::s, &B<int>::v}; }
