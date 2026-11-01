// Clang extension: the lock-free queries fold as constants, and _Atomic
// rounds a small size up to a power of two (AArch64: 16-byte inline atomics).
//@ expect: compile-ok
//@ args: --target=aarch64-unknown-linux-gnu -DARCH16=1 -DLD=1
struct S3 { char c[3]; };
struct S16 { long a, b; };
struct S32 { long a[4]; };
static_assert(__atomic_always_lock_free(1, 0));
static_assert(__atomic_always_lock_free(8, 0));
static_assert(!__atomic_always_lock_free(3, 0));
static_assert(!__atomic_always_lock_free(32, 0));
static_assert(__c11_atomic_is_lock_free(4));
static_assert(__atomic_is_lock_free(8, 0));
static_assert(__atomic_always_lock_free(sizeof(S16), 0) == ARCH16);
static_assert(__atomic_always_lock_free(4, (int *)8));
constexpr bool b1 = __atomic_always_lock_free(sizeof(long double), 0);
static_assert(b1 == LD);
static_assert(sizeof(_Atomic(S3)) == 4 && alignof(_Atomic(S3)) == 4);
static_assert(sizeof(_Atomic(S16)) == 16 && alignof(_Atomic(S16)) == 16);
static_assert(sizeof(_Atomic(S32)) == 32 && alignof(_Atomic(S32)) == 8);
static_assert(sizeof(_Atomic(char)) == 1 && alignof(_Atomic(long long)) == 8);
