// [implimits]: a wrapper of the evaluation context outside namespace std is
// an ordinary call, its statements counted as clang counts them (spec/oven.md
// "Library fast paths"): this loop needs 505 steps on kiln and clang.
//@ args: -fconstexpr-steps=505
//@ expect: compile-ok
namespace lib {
constexpr bool in_constant_context() {
  if consteval {
    return true;
  } else {
    return false;
  }
}
} // namespace lib
constexpr int count(int n) {
  int k = 0;
  for (int i = 0; i < n; ++i)
    k += lib::in_constant_context();
  return k;
}
static_assert(count(100) == 100);
