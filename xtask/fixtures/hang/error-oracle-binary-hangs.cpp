// The oracle (also clang 19 here) builds `oracle.out`; only that binary
// hangs, so the reference is broken and the result is ERROR, not FAIL.
//@ expect: run
#include <cstring>
int main(int, char** argv) {
  volatile bool oracle = std::strstr(argv[0], "oracle") != nullptr;
  while (oracle) {
  }
}
