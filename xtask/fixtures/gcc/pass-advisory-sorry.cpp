//@ expect: run
//@ oracle: gcc
//@ stdout: 7
// fake-gcc: sorry
#include <cstdio>
int main() { std::printf("7\n"); }
