// [implimits]: one step fewer than the loop of light-frames.cpp needs
// crosses the limit (spec/oven.md "Light frames": a light call's steps
// are a call's).
//@ args: -fconstexpr-steps=305
//@ expect: compile-fail
//@ skip(clang): clang counts steps by its own rule
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 16:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 18:15 note "in call to 'sum(100)'"
constexpr int get(const int *p) { return *p; }
constexpr int sum(int n) {
  int a[4] = {1, 2, 3, 4};
  int s = 0;
  for (int i = 0; i < n; ++i)
    s += get(&a[i % 4]);
  return s;
}
static_assert(sum(100) == 250);
