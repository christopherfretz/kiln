// Without `oven:`, kiln runs the test like any compiler, with no report flag.
//@ expect: run
//@ stdout: 16
#include <cstdio>
constexpr int sq(int x) { return x * x; }
int main() { std::printf("%d\n", sq(4)); }
