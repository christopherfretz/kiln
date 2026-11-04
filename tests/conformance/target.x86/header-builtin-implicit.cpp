// [basic.lookup.unqual] clang's x86 header builtins (`_mm_pause`,
// `_mm_mfence`, ...) are not predefined library functions, so C++ lookup
// declares one implicitly, as clang's `LazilyCreateBuiltin` does, with
// `-Wimplicit-function-declaration` and the header that declares it
// (spec/landed/sema/WP-630.md "Header builtins"). WP-630.
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 10:12 warning "implicitly declaring library function '_mm_pause' with type 'void () noexcept'"
//@ diag: 10:12 note "include the header <emmintrin.h> or explicitly provide a declaration for '_mm_pause'"
void f() { _mm_pause(); }
