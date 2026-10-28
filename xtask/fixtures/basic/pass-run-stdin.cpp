//@ expect: run
//@ run-stdin: first line
//@ run-stdin:
//@ run-stdin: third
//@ stdout: 3 lines, 18 bytes
#include <cstdio>
int main() {
  int lines = 0, bytes = 0;
  for (int c; (c = std::getchar()) != EOF; ++bytes) lines += c == '\n';
  std::printf("%d lines, %d bytes\n", lines, bytes);
}
