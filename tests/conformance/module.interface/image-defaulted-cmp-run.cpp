// [class.compare.default]/1-6, [class.eq]/2-3, [module.interface],
// spec/modules.md §11.11 (WP-1314): an imported module's defaulted
// comparison operators behave exactly as with the replayed source under
// `--bmi-image`: subobject comparisons through the operators found at
// the `= default`, implicit exception specifications, constant
// evaluation.
//@ expect: run
//@ std: c++20
//@ module: imgdc=Inputs/img-defaulted-cmp.cppm
import imgdc;
static_assert(noexcept(dc::tc{} == dc::tc{}) && !noexcept(dc2::U{} == dc2::U{}));
bool g() { return dc2::U{{1}} == dc2::U{{1}}; }
static_assert(dc::po{2} == dc::po{2} && dc::po{2} != dc::po{3});
static_assert(dc::box<int>{4} == dc::box<int>{4});
static_assert(noexcept(dc::po{} == dc::po{}));
int main() {
  dc::tc a{1, 2}, b{1, 3};
  dc::mm m{1}, n{1};
  dc::box<long> x{5}, y{6};
  return !g() + (a == b) + (m != n) + (x == y) + !dc::used;
}
