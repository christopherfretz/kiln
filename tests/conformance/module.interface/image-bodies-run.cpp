// [module.interface], [dcl.constexpr]/1, [expr.const]/5, spec/modules.md
// §11.11 "Part 2a" (stage S2): inline, constexpr and consteval function
// bodies and variable initializers exported from a module. The importer
// constant-evaluates them (static_assert) and runs them; under
// `--bmi-image` it builds every body from the declaration image, and the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgb=Inputs/img-bodies.cppm
//@ stdout: 120 49 0 10 5 | 100 24 26 -1 | 11 4 30 | 10 fname 13 10 1 | 3 7 5 0.5 1 | 245 A
import imgb;
extern "C" int printf(const char *, ...);
using namespace ib;
static_assert(fact(5) == 120 && kAnswer == 120 && sq(7) == 49);
static_assert(classify(0) == 100 && classify(1) == 22 && classify(3) == 6 && classify(9) == -1);
static_assert(loops(5) == 11 && length(kName) == 4 && length("") == 0);
static_assert(mix(3) == 9 + 3 + 1 + 4 + 1 + 1 + 1 + 0 + 2 + 1);
static_assert(with_default(1) == 10 && call_default(2) == 11 && with_default(1, 1) == 2);
static_assert(kArr[2] == 6 && kHalf == 0.5 && kOn && kLimit == 7);
static_assert(flip(10u) == 245u && upper('a') == 'A' && upper('Z') == 'Z');

int main(int argc, char **) {
  int n = argc + 4;
  printf("%d %d %d %d %d | ", fact(n), sq(7), clamp3(-n), clamp3(n * 3), clamp3(n));
  printf("%d %d %d %d | ", classify(n - 5), classify(n - 3), classify(n - 2) + 20, classify(n));
  printf("%d %d %d | ", loops(n + 1), length(kName), checked(n * 6));
  printf("%d %s %d %d %d | ", (int)mix(n - 5) - 3, fname() + 0 * n, stmt_expr(n - 1),
         jump(n), call_default(n - 4) - 9);
  printf("%d %d %d %g %d | ", counter, kLimit, kArr[1], kHalf, kOn);
  printf("%u %c\n", flip(10u + 0 * n), upper('a' + 0 * n));
}
