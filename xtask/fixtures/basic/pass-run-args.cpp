//@ expect: run
//@ run-args: one "two words" 'it''s'
//@ run-args: last\ arg
//@ stdout: 5
//@ stdout: [one]
//@ stdout: [two words]
//@ stdout: [its]
//@ stdout: [last arg]
#include <cstdio>
int main(int argc, char** argv) {
  std::printf("%d\n", argc);
  for (int i = 1; i < argc; ++i) std::printf("[%s]\n", argv[i]);
}
