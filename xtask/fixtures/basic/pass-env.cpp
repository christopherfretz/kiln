//@ expect: run
//@ env: KILN_FIXTURE_A=x=1
//@ env: KILN_FIXTURE_B=
//@ stdout: [x=1] [] [-]
#include <cstdio>
#include <cstdlib>
static const char* get(const char* n) {
  const char* v = std::getenv(n);
  return v ? v : "-";
}
int main() {
  std::printf("[%s] [%s] [%s]\n", get("KILN_FIXTURE_A"), get("KILN_FIXTURE_B"),
              get("KILN_FIXTURE_UNSET"));
}
