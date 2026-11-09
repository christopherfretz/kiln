// [cpp.include]/8 (implementation-defined nesting limit), [implimits]: a
// file that includes itself twice is a cycle with two back edges, so
// entering every path up to the depth limit (200) takes 2^200 inclusions.
// Like clang (`HasReachedMaxIncludeDepth`), once the limit is reached a file
// already entered is not entered again: one error at the deepest level,
// then the cycle unwinds. kiln walked every path and never finished (WP-971).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:10 error "#include nested too deeply"
#if __INCLUDE_LEVEL__ < 300
#include "include-cycle-two-back-edges.cpp"
#include "include-cycle-two-back-edges.cpp"
#endif
#if __INCLUDE_LEVEL__ == 0
int main() {}
#endif
