// [implimits]: a call of a function that makes no frame object (a light
// frame, spec/oven.md "Light frames") costs the steps of any call: this
// loop needs 306 steps on kiln with and without light frames.
//@ args: -fconstexpr-steps=306
//@ expect: compile-ok
//@ skip(clang): clang counts steps by its own rule
constexpr int get(const int *p) { return *p; }
constexpr int sum(int n) {
  int a[4] = {1, 2, 3, 4};
  int s = 0;
  for (int i = 0; i < n; ++i)
    s += get(&a[i % 4]);
  return s;
}
static_assert(sum(100) == 250);
