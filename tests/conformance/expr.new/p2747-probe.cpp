// [expr.const]/5.18 (C++26, P2747R2): a non-allocating placement new may be
// evaluated in a constant expression when the pointer refers to storage of a
// suitable type. constexpr-placement.cpp pins the pre-P2747 (clang 19)
// rejection. WP-185 feature probe.
//@ expect: compile-ok
//@ strict-diags
//@ xfail(clang): clang 19 lacks P2747 (clang 20+ accepts)
//@ xfail(kiln): placement new rejected in constant evaluation
namespace std { using size_t = decltype(sizeof 0); }
constexpr void* operator new(std::size_t, void* p) noexcept { return p; }
constexpr int place() {
  int x = 1;
  int* p = ::new (&x) int(42);
  return *p;
}
static_assert(place() == 42);
