// [module.interface]/2, [dcl.enum]/2, spec/modules.md §11.11 "Part 2a":
// a module whose interface the declaration image carries entirely
// (namespaces, export blocks, functions with and without bodies,
// enumerations scoped, unscoped and fixed, typedefs and alias
// declarations, function pointer and array types). Under `--bmi-image`
// the importer builds `img` from the image; the program behaves exactly
// as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: img=Inputs/img.cppm
//@ stdout: 3 5 -2 1 6 7 9 2 1
import img;
extern "C" int printf(const char *, ...);
static_assert(static_cast<int>(geo::Color::green) == 5 && static_cast<int>(geo::Color::blue) == 6);
static_assert(geo::neg == -3 && geo::pos == 1 && sizeof(geo::Color) == 1);
static_assert(geo::big == 0xffffffffffffull && sizeof(geo::Wide) == 8);
static_assert(noexcept(geo::bump(1)) && !noexcept(geo::add(1, 2)));

int main() {
  geo::Int i = geo::add(1, 2);
  geo::IntPtr p = &i;
  geo::Fn f = geo::pick();
  Quad q = {7, 8, 9, 10};
  printf("%d %d %d %d %ld %d %d %ld %d\n", *p, static_cast<int>(geo::Color::green),
         geo::Sign::neg + 1, geo::pos, twice(3), first(q), f(4, 5), sink(1),
         geo::bump(0));
}
