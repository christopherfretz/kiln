// [basic.start.main]: argc/argv reflect the program arguments.
//@ expect: run
//@ run-args: x
//@ stdout: 2 x
#include <cstdio>

int main(int argc, char** argv) { std::printf("%d %s\n", argc, argc > 1 ? argv[1] : "-"); }
