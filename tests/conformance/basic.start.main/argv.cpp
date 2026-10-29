// [basic.start.main]: argc/argv reflect the program arguments.
//@ expect: run
//@ run-args: x
//@ stdout: 2 x
//@ xfail(kiln): libstdc++ headers do not parse yet: kiln_parse does not support decltype (c++config.h)
#include <cstdio>

int main(int argc, char** argv) { std::printf("%d %s\n", argc, argc > 1 ? argv[1] : "-"); }
