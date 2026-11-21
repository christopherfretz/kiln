// [module.interface], spec/modules.md §11.11 "Dependent types"
// (WP-1811): clang address spaces exported from a module, written
// with a number (`ExtQual`) and with a value-dependent `address_space(N)`
// (clang DependentAddressSpaceType) in alias templates, function
// templates that deduce N and static data members; under `--bmi-image`
// they come from the declaration image and the program behaves exactly
// as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgas=Inputs/img-address-space.cppm
//@ stdout: 7 8 1 2 71 0 1 7 6
import imgas;
extern "C" int printf(const char *, ...);
template <class A, class B> constexpr bool same_v = false;
template <class A> constexpr bool same_v<A, A> = true;
static_assert(same_v<as::holder<1>::type, as::as1_int>);
static_assert(same_v<as::sp<2>, int __attribute__((address_space(2)))>);
static_assert(!same_v<as::sp<1>, as::sp<2>>);

__attribute__((address_space(1))) int g1 = 7;
__attribute__((address_space(2))) int g2 = 7;
int g0 = 4;

int main() {
  printf("%d %d %d %d %d %d %d %d %d\n", as::load(&g1), as::read(&g1), as::which(&g1),
         as::which(&g2), as::next<1>(&g2), as::pick(&g0), as::pick(&g1),
         as::read<1>(&g1) - 1, int(as::box<2>::v));
  return 0;
}
