// [WP-3698] Constant and dynamic `ext_vector_type` bool splats into wide
// `_BitInt(N)` lanes (N > 128; x86-64 only, aarch64 caps `_BitInt` at 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang>=22
//@ expect: compile-ok
using S = _BitInt(256);
typedef S V2s __attribute__((ext_vector_type(2)));

constexpr V2s ct = (V2s)true;
constexpr V2s cf = (V2s)false;
static_assert(ct[0] == -1 && ct[1] == -1);
static_assert(cf[0] == 0 && cf[1] == 0);

constexpr V2s mk(bool b) { return (V2s)b; }
static_assert(mk(true)[0] == -1 && mk(false)[1] == 0);

bool bb;
V2s dyn = (V2s)bb;
