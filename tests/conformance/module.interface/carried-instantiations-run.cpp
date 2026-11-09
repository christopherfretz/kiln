// [temp.inst], [module.interface], spec/modules.md §11.11 Part 2 (WP-1046):
// an importer uses templates whose instantiations the module made. A
// kind-2 module file carries the constant results of those
// instantiations' static_asserts, constexpr variables, `if constexpr`
// conditions and the non-type template arguments derived from them; the
// importer sees the same values as with kind 1 (`--bmi-kind 1|2`).
//@ expect: run
//@ std: c++20
//@ module: ct=Inputs/ct.cppm
//@ stdout: 6765 4181 3194 3 988 -4 610 1 | 6765 3 610 55
import ct;
extern "C" int printf(const char *, ...);

constexpr long n = Fib<20>::value;
static_assert(use == 6765 + 2584 + 3194 + (4181 % 7 + 1));
static_assert(use2 == 988 - 4 + 610);
static_assert(Fib<20>::big && big20 && !Fib<5>::big);
static_assert(twice<17>() == 3194 && twice<3>() == -4);
static_assert(Arr<int, 19>::size == 4181 % 7 + 1 && Tag<Fib<20>::value>::value == n);
Arr<char, 19> arr{};
FibTag<15> tag15;

int main() {
  if constexpr (Fib<19>::big)
    printf("%ld %ld %ld %d %ld %ld %ld %d | ", Fib<20>::value, Fib<19>::value, twice<17>(),
           Arr<int, 19>::size, vt<16>, twice<3>(), FibTag<15>::value, arr.first());
  printf("%ld %d %ld %ld\n", n, int(sizeof(arr.data)), decltype(tag15)::value, Fib<10>::value);
}
