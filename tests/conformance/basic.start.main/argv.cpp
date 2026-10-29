// [basic.start.main]: argc/argv reflect the program arguments.
//@ expect: run
//@ run-args: x
//@ stdout: 2 x
//@ xfail(kiln): libstdc++ headers do not parse yet: function overloading (c++config.h), __builtin_va_list and asm labels (stdio.h)
#include <cstdio>

int main(int argc, char** argv) { std::printf("%d %s\n", argc, argc > 1 ? argv[1] : "-"); }
