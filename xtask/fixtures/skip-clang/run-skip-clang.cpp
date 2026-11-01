//@ expect: run
//@ skip(clang): stands for a test clang cannot compile; the oracle is a clang, so it is not run
//@ stdout: ok
#include <cstdio>
int main() { std::puts("ok"); }
