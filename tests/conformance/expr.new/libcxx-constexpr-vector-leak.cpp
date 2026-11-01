// [expr.const]/5.20: storage std::allocator<T>::allocate obtains (here
// through std::vector, whose destructor never runs) must be deallocated
// within the evaluation. spec/oven.md "Transient allocation".
//@ expect: compile-fail
//@ args: -stdlib=libc++
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:16 error "constexpr variable 'leaked' must be initialized by a constant expression"
#include <memory>
#include <vector>

constexpr int leak() {
  auto *v = new std::vector<int>{1, 2, 3};
  return (*v)[2];
}
static_assert(leak() == 3);

constexpr int *keep() {
  std::allocator<int> a;
  return a.allocate(4);
}
constexpr int *leaked = keep();
