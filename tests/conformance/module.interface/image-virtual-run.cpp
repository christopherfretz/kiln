// [module.interface], [class.virtual], [expr.const]/5, spec/modules.md
// §11.11 "Part 2a": virtual calls across a module boundary, and an
// importer overriding an imported virtual function. Under
// `--bmi-image` the classes come from the declaration image (WP-1120),
// with the replay's behaviour.
//@ expect: run
//@ std: c++20
//@ module: imgv=Inputs/img-virtual.cppm
//@ stdout: 25 6 4 3 8 | 9 5 18
import imgv;
extern "C" int printf(const char *, ...);
using namespace iv;
static_assert(kSq == 25 && area_of(Tri(3, 4)) == 6 && Square(2).twice() == 8);

// Cross-module inheritance with an override.
struct Pent : Shape {
  int k;
  constexpr explicit Pent(int n) : k(n) {}
  constexpr int area() const override { return k + 4; }
  constexpr int sides() const override { return 5; }
};
static_assert(area_of(Pent(5)) == 9 && Pent(1).twice() == 10);

int main(int argc, char **) {
  Square sq(argc + 4);
  Tri t(argc + 2, 4);
  Pent p(argc + 4);
  const Shape *all[] = {&sq, &t, &p};
  printf("%d %d %d %d %d | ", area_of(sq), area_of(t), dyn_sides(all[0]), dyn_sides(all[1]),
         Square(argc + 1).twice());
  printf("%d %d %d\n", all[2]->area(), dyn_sides(all[2]), p.twice());
}
