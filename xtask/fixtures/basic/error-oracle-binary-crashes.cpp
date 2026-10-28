// Only the oracle's binary (`oracle.out`) crashes: ERROR, not FAIL.
//@ expect: run
#include <cstdlib>
#include <cstring>
int main(int, char** argv) {
  if (std::strstr(argv[0], "oracle")) std::abort();
}
