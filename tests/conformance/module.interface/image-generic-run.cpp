// [module.interface], [expr.const]/5, [temp.inst]/11, [class.copy.elision]/3, spec/modules.md
// §11.11 "Part 2a" (WP-1132): bodies using `_Generic` (clang's
// GenericSelectionExpr, a C11 extension in C++) and
// `__builtin_choose_expr` (ChooseExpr) exported from a module. The
// importer constant-evaluates them, instantiates the templates (dependent
// controls, association types and conditions, a returned parameter moved
// through either node) and runs them; under
// `--bmi-image` every body comes from the declaration image, and the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgg=Inputs/img-generic.cppm
//@ stdout: 1 3 14 | 9 7 | 1 2 3 0 1 0 | 5 | 6 60 6 | 8 | 100 100
import imgg;
extern "C" int printf(const char *, ...);
int T = 4, N = 1; // the templates' parameter names: not gated
static_assert(ig::twice(21) == 42);
static_assert(ig::tag(1) == 1 && ig::tag('c') == 2 && ig::tag(1.0) == 3 && ig::tag(1L) == 0);
static_assert(ig::is<int>(0) == 1 && ig::is<long>(0) == 0);
static_assert(ig::choose(4) == 5 && ig::choose_t<1>(4) == 40 && ig::choose_t<0>(4) == 4);
static_assert(ig::choose_size() == sizeof(long));
// Copy adds 1, move adds 100: the returned parameter is moved out.
struct Only {
  int v;
  constexpr Only(int v) : v(v) {}
  constexpr Only(const Only &o) : v(o.v + 1) {}
  constexpr Only(Only &&o) : v(o.v + 100) {}
};
static_assert(ig::gmove(Only(0)).v == 100 && ig::cmove(Only(0), Only(5)).v == 100);

int main(int argc, char **) {
  int n = argc + 6; // 7
  printf("%d %d %d | ", ig::kind(n), ig::kind(1.5 * n), ig::twice(n));
  long l = 0;
  ig::pick(n, l) = 9; // n = 9
  int m = 7;
  printf("%d %d | ", n, m);
  printf("%d %d %d %d %d %d | ", ig::tag(n), ig::tag('a'), ig::tag(0.5), ig::tag(l),
         ig::is<int>(n), ig::is<char>(n));
  int a = 1, b = 2;
  ig::first(a, b) = 5;
  printf("%d | ", a);
  ig::choose_lv(a, b) = 6;
  printf("%d %d %d | ", a, ig::choose_t<1>(a), ig::choose_t<0>(a));
  printf("%d | ", ig::choose(m));
  printf("%d %d\n", ig::gmove(Only(argc - 1)).v, ig::cmove(Only(argc - 1), Only(argc)).v);
  return 0;
}
