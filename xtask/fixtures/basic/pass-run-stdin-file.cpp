//@ expect: run
//@ run-stdin-file: Inputs/stdin.txt
//@ stdout: 3 lines
#include <cstdio>
int main() {
  int lines = 0;
  for (int c; (c = std::getchar()) != EOF;) lines += c == '\n';
  std::printf("%d lines\n", lines);
}
