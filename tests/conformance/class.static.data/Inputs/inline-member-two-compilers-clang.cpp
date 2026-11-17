// The unit of ../inline-member-two-compilers-*-run.cpp that clang compiles.
#include "inline_member_two_compilers.h"
Units from_clang() { return {&A::n, &A::m, &A::s, &B<int>::v}; }
