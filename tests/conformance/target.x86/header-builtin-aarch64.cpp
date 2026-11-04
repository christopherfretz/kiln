// [dcl.link] On AArch64 `_mm_pause` names no builtin: a declaration of it
// is an ordinary function, as in clang (spec/landed/sema/WP-630.md
// "Header builtins"). WP-630.
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
extern "C" void _mm_pause(void);
extern "C" int _mm_getcsr(int, int);
void f() { _mm_pause(); (void)_mm_getcsr(1, 2); }
