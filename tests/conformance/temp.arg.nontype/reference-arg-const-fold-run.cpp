// [temp.arg.nontype]: a reference template parameter bound to a const
// global reads its value (WP-998 emits the folded constant).
//@ expect: run
//@ std: c++20
//@ stdout: 3 4 6
#include <cstdio>
static const int cg = 3;
const int eg = 4;
static const long cl = 5;
template <const int &r> int ref() { return r; }
template <const long &r> long ref_long() { return r + 1; }
template <const int &r> const int *addr() { return &r; }
int main() {
  if (addr<cg>() != &cg)
    return 1;
  std::printf("%d %d %ld\n", ref<cg>(), ref<eg>(), ref_long<cl>());
}
