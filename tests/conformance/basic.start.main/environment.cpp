// [cstdlib.syn]: getenv sees the environment the program was started with.
//@ expect: run
//@ env: KILN_GREETING=hello
//@ run-stdin: world
//@ stdout: hello world
//@ xfail(kiln): libstdc++ headers do not parse yet: kiln_parse does not support decltype (c++config.h)
#include <cstdio>
#include <cstdlib>

int main() {
  char word[16] = {};
  if (std::scanf("%15s", word) != 1) return 1;
  std::printf("%s %s\n", std::getenv("KILN_GREETING"), word);
}
