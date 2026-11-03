// [ACLE] [driver] With the `sve` target feature (`-march=armv8-a+sve`,
// `-mcpu=generic+sve`, or `-Xclang -target-feature -Xclang +sve`), SVE
// types are usable: automatic variables, parameters, returns and the
// vector operators are valid, with no "cannot be used in a target without
// sve" error. Sizeless restrictions still hold (a non-local variable, a
// `sizeof`). spec/driver.md §Target features, spec/types.md §5.1.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve -ferror-limit=0
//@ strict-diags
//@ diag: 28:12 error "non-local variable with sizeless type 'i32' (aka '__SVInt32_t')"
//@ diag: 29:9 error "invalid application of 'sizeof' to sizeless type 'i32' (aka '__SVInt32_t')"

typedef __SVInt32_t i32;
typedef __SVBool_t b;

i32 add(i32 a, i32 x) {
  i32 r = a + x;
  r = r * a - x;
  i32 m = a < x;
  (void)m;
  i32 s = a < x ? a : x;
  i32 *p = &s;
  return *p & r;
}

i32 pass(i32 a) { return add(a, a); }

static i32 g;
int n = sizeof(i32);
