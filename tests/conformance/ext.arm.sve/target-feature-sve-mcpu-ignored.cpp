// [driver] When `-march=` is given, `-mcpu=` does not contribute target
// features (clang's AArch64 driver uses the CPU only without an
// architecture), so `-mcpu=generic+sve -march=armv8-a` has no `sve`.
// spec/driver.md §Target features.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -mcpu=generic+sve -march=armv8-a
//@ strict-diags
//@ diag: 11:31 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 11:6 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"

void f(__SVInt32_t a) { (void)a; }
