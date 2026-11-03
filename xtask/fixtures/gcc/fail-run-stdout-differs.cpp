//@ expect: run
//@ oracle: gcc
#include <cstdio>
int main() {
#ifdef KILN_FAKE_GCC
  std::printf("gcc\n");
#else
  std::printf("kiln\n");
#endif
}
