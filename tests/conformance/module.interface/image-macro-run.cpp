// [module.interface]/2, [cpp.replace]/10, spec/modules.md §11.11
// "Expansion records" (stage S7): declarations produced by macro
// expansions (function-like macros, macro arguments, token pasting,
// nested and variadic expansions). Under `--bmi-image` the importer
// builds `imgm` from the image, the declarations' locations in its
// expansion records; the program behaves exactly as with the replay.
//@ expect: run
//@ std: c++20
//@ module: imgm=Inputs/img-macro.cppm
//@ stdout: 3 6 105 9 -2 5 103
import imgm;
extern "C" int printf(const char *, ...);
static_assert(inc(1) == 2 && edge_lo(1) == 2 && edge_hi(1) == 101 && kSum == 103);
static_assert(static_cast<int>(Dir::south) == 5 && Cell{4}.get() == 4 && negate(3) == -3);

int main() {
  printf("%d %d %d %d %d %d %d\n", inc(2), edge_lo(5), edge_hi(5), Cell{9}.get(), negate(2),
         static_cast<int>(Dir::south), kSum);
}
