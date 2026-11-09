// [module.interface], [basic.def.odr]/14, [dcl.inline]/6, [stmt.dcl]/4,
// spec/modules.md §11.11 "Part 2a" (stage S2): an inline function
// exported from a module, with a static local, is one entity across the
// module and two importers (one static, one address of `greeting`'s
// array). Further literal, operator and pointer kinds are evaluated
// from the module's bodies; under `--bmi-image` from the image.
//@ expect: run
//@ std: c++20
//@ module: imgc=Inputs/img-bodies2.cppm
//@ unit: Inputs/img-bodies2-user.cpp
//@ stdout: 1 20 3 1 hi | 7 1 33 0.85 9 4 6 abcdef
import imgc;
extern "C" int printf(const char *, ...);
extern "C" unsigned long strlen(const char *);
int user_next();
const char *user_greeting();
static_assert(ic::widths() == 4 + 1 + 2 + 4 + 1);
static_assert(ic::choose() == 7 && ic::arrays(1) == 0 + 4 + 3 + 2 + 2);
static_assert(ic::halves(1.0) == 0.5 + 0.25 + 0.1);
static_assert(ic::big() == 0x0fffffffffffffffull && ic::neg_mod(5, 3) == -2 + 1 - 0 + 0);
static_assert(ic::ptrs() == 2 + 3 + 1 + 1);
int buf[ic::kBound];
static_assert(sizeof(buf) == 4 * sizeof(int) && ic::kWords[5] == 'f');

int main() {
  int a = ic::next();
  int b = user_next();
  int c = ic::next();
  printf("%d %d %d %d %s | ", a, b, c, user_greeting() == ic::greeting(), ic::greeting());
  printf("%d %d %d %g %d %d %d %s\n", ic::choose(), ic::null_is_zero(), ic::arrays(3),
         ic::halves(1.0), ic::ptrs() + 2, (int)sizeof(buf) / (int)sizeof(int),
         (int)strlen(ic::kWords), ic::kWords);
}
