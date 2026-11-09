// [temp.fct], [temp.param], [temp.inst]/11, [module.interface],
// spec/modules.md §11.11 "Templates as patterns" (stage S4): function
// templates exported from a module, instantiated by the importer with
// type and non-type arguments (deduced and explicit), in constant
// evaluation and at run time; under `--bmi-image` every template is
// rebuilt from the declaration image and instantiated from that pattern,
// and the program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgt=Inputs/img-template.cppm
//@ stdout: 5 3.5 12 10 25 42 7 8 2 9 15 6766 6765 | 4 1
import imgt;
extern "C" int printf(const char *, ...);
int T = 4, N = 1; // the templates' parameter names: not gated
static_assert(it::add(2, 3) == 5 && it::scale<int, 3>(4) == 12 && it::square<5>() == 25);
static_assert(it::width<char>() == 1 && it::width<int>() == sizeof(int));
struct P {
  int v;
  P &operator+=(P o) {
    v += o.v;
    return *this;
  }
};

int main() {
  int v = 7;
  P p = it::twice(P{21});
  printf("%d %g %d %ld %d %d %d ", it::add(2, 3), it::add(1.5, 2.0), it::scale<int, 3>(4),
         it::scale<long, 2>(5L), it::square<5>(), p.v, *it::addr(v));
  printf("%d %d %d %d %ld %d | ", it::pick(8, 1, true), it::pick(2, 0),
         (int)it::narrow<unsigned char>(265), it::sum_to(5), it::heavy(1L), it::heavy_one() - 1);
  printf("%d %d\n", T, N);
  return 0;
}
