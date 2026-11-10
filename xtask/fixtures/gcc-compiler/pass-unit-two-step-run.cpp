//@ expect: run
//@ unit: Inputs/unit-five.cpp
//@ args: -Qunused-arguments -Rpass-missed=inline -Wunreachable-code-aggressive
//@ args: -Werror=unused-variable -fno-crash-diagnostics
//@ stdout: 5
// A `unit:` test runs under GCC (WP-1261): the unit compile and the link
// get the test's flags in GCC's spelling, the clang-only ones dropped
// (`-Qunused-arguments`, `-R<remark>`, a warning GCC does not list, and
// `-fno-crash-diagnostics`), and the link names libstdc++exp last.
#include <cstdio>
int five();
int main() { std::printf("%d\n", five()); }
