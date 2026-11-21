// [cfenv.syn], C23 7.6.2: the values a directed `#pragma STDC FENV_ROUND`
// gives constant initializers reach the program: a static double and a
// constexpr float initialized in FE_UPWARD and FE_DOWNWARD scopes hold the
// rounded-up and rounded-down quotients, and a function-scope constexpr
// in a FE_TOWARDZERO block holds the truncated one, at -O0 and after the
// optimizer (WP-1744).
//@ expect: run
//@ args: -O2 -Wno-unknown-pragmas
//@ stdout: up 3fd5555555555556
//@ stdout: down 3fd5555555555555
//@ stdout: upf 3eaaaaab
//@ stdout: downf 3eaaaaaa
//@ stdout: zero 3fb9999999999999
//@ stdout: tie 9007199254740994
#include <cstdio>
#include <cstring>

static unsigned long long bits(double d) {
  unsigned long long u;
  std::memcpy(&u, &d, sizeof u);
  return u;
}

static unsigned bitsf(float f) {
  unsigned u;
  std::memcpy(&u, &f, sizeof u);
  return u;
}

#pragma STDC FENV_ROUND FE_UPWARD
static double up = 1.0 / 3.0;
constexpr float upf = 1.0f / 3.0f;
#pragma STDC FENV_ROUND FE_DOWNWARD
static double down = 1.0 / 3.0;
constexpr float downf = 1.0f / 3.0f;
#pragma STDC FENV_ROUND FE_TONEARESTFROMZERO
static double tie = 0x1p53 + 1.0;
#pragma STDC FENV_ROUND FE_TONEAREST

int main() {
  double zero;
  {
#pragma STDC FENV_ROUND FE_TOWARDZERO
    constexpr double z = 1.0 / 10.0;
    zero = z;
  }
  std::printf("up %llx\n", bits(up));
  std::printf("down %llx\n", bits(down));
  std::printf("upf %x\n", bitsf(upf));
  std::printf("downf %x\n", bitsf(downf));
  std::printf("zero %llx\n", bits(zero));
  std::printf("tie %.0f\n", tie);
}
