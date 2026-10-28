// Smoke-test entry for `cargo xtask bench` (xtask/src/bench/tests.rs).
//@ mode: emit-llvm
//@ std: c++20
constexpr int answer() { return 6 * 7; }
static_assert(answer() == 42);
int main() { return answer() - 42; }
