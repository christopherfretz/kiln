// sides: pre post
// scope: any
// Every attribute of the hash's closed list (spec/abi.md "Attributes") and
// three that are not on it, on unrelated declarations; an abi tag.
struct alignas(32) KcAligned { int x; };
struct KcAligned2 { int y; } __attribute__((aligned(16)));
[[noreturn]] void kc_die();
__attribute__((noreturn)) void kc_die2();
#if defined(__aarch64__)
__attribute__((target("arch=armv8.2-a"))) inline int kc_target() { return 1; }
#elif defined(__x86_64__)
__attribute__((target("avx2"))) inline int kc_target() { return 1; }
#endif
#if __cplusplus > 202302L
void kc_indeterminate([[indeterminate]] int x);
#endif
inline int kc_assume(int x) { [[assume(x > 1)]]; return x; }
[[nodiscard]] int kc_nodiscard();
[[deprecated]] int kc_deprecated();
[[gnu::cold]] int kc_cold();
__attribute__((abi_tag("kctag"))) inline int kc_tagged() { return 1; }
