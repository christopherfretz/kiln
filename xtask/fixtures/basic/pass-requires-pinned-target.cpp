//@ requires: target-x86_64
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
// WP-239: the pinned `--target` meets the requirement on every host (an
// aarch64 host used to skip it).
static_assert(sizeof(long) == 8 && __x86_64__ == 1);
