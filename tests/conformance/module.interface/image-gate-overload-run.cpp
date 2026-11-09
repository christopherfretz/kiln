// [basic.def.odr]/15, [namespace.def]/2, [over.match], [basic.link],
// spec/modules.md §11.11 "Part 2a", "ODR merging" (WP-1162): the importer
// declares, at global namespace scope and before `import imgs;`, an
// overload of the module's `twice` and its own members of the module's
// namespace `geo2`, inside a linkage specification. Those meet the
// module's declarations, so under `--bmi-image` the unit still replays:
// `twice` names both overloads and `geo2` is one namespace holding the
// importer's and the module's members.
//@ expect: run
//@ std: c++20
//@ module: imgs=Inputs/img-gate-scope.cppm
//@ stdout: 6 103 12 3 20
int twice(double x) { return static_cast<int>(x) + 100; }
extern "C++" {
namespace geo2 {
int perimeter(int w, int h) { return 2 * (w + h); }
} // namespace geo2
}
import imgs;
extern "C" int printf(const char *, ...);
static_assert(geo2::area(3, 4) == 12);
int main() {
  printf("%d %d %d %d %d\n", twice(3), twice(3.5), geo2::area(3, 4), geo2::count,
         geo2::perimeter(4, 6));
}
