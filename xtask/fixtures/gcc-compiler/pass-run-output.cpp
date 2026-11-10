//@ expect: run
//@ stdout: 7
//@ args: -ferror-limit=0
#include <cstdio>
int main() { std::printf("7\n"); }
