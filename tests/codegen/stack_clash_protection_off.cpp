// WP-1411: the last of `-fstack-clash-protection` /
// `-fno-stack-clash-protection` wins, and clang's default probe size
// (4096) writes no "stack-probe-size" (spec/driver.md "Codegen flags").
//@ args: -fstack-clash-protection -fno-stack-clash-protection -mstack-probe-size=4096
int f();
int g = f();
int big(int n) { char buf[100000]; buf[n] = 1; return buf[n / 2]; }
