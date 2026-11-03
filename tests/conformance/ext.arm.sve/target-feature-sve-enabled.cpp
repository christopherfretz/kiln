// [driver] `-Xclang -target-feature -Xclang +sve` (here after `-mcpu=generic`)
// enables the `sve` feature as `-march=armv8-a+sve` does; SVE parameters,
// locals and operators are then valid. spec/driver.md §Target features.
//@ expect: compile-ok
//@ args: --target=aarch64-unknown-linux-gnu -mcpu=generic -Xclang -target-feature -Xclang +sve

__SVInt32_t f(__SVInt32_t a, __SVInt32_t b) {
  __SVInt32_t r = a + b;
  return r < a ? r : a;
}
