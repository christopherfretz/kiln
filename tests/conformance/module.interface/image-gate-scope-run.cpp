// [basic.def.odr]/15, [basic.scope.namespace], [class.mem], [cpp.replace],
// spec/modules.md §11.11 "Part 2a", "ODR merging" (WP-1162): before
// `import imgs;` the importer spells the module's names without declaring
// them at global namespace scope: in a macro, as members of its own class,
// in its own namespace, as a local, and declares `count` at global scope
// while the module's `count` is `geo2::count`. None of that can merge
// with the module's declarations, so under `--bmi-image` the unit is built
// from its image (the driver test `kind2_image_gate_module_names` checks
// the `-v` note); the program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgs=Inputs/img-gate-scope.cppm
//@ stdout: 6 4 6 101 7 5 3 1 2
#define TWICE(x) twice(x)
struct Holder {
  int twice;
  int get;
};
namespace other {
int Box = 7;
int twice(int x) { return x + 100; }
} // namespace other
int count = 5;
int local() {
  int area = 0;
  return area;
}
import imgs;
extern "C" int printf(const char *, ...);
static_assert(Box{4}.get() == 4 && geo2::area(2, 3) == 6);
int main() {
  Holder h{1, 2};
  printf("%d %d %d %d %d %d %d %d %d\n", TWICE(3), Box{4}.get(), geo2::area(2, 3),
         other::twice(1), other::Box + local(), count, geo2::count, h.twice, h.get);
}
