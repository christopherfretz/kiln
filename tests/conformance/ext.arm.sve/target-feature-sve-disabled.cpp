// [driver] The last setting of a target feature wins: a later
// `-target-feature -sve` disables the `sve` that `-march=armv8-a+sve`
// enabled, so SVE types are unavailable again. spec/driver.md §Target
// features.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve -Xclang -target-feature -Xclang -sve
//@ strict-diags
//@ diag: 11:31 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 11:6 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"

void f(__SVInt32_t a) { (void)a; }
