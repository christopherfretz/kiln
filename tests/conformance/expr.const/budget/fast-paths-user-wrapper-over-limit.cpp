// [implimits]: a wrapper of the evaluation context outside namespace std is
// an ordinary call, its statements counted as clang counts them (spec/oven.md
// "Library fast paths"): this loop needs 505 steps on kiln and clang.
//@ args: -fconstexpr-steps=504
//@ expect: compile-fail
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 24:15 note "in call to 'count(100)'"
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
