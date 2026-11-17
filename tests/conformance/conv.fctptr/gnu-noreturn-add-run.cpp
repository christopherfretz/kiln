// GNU: a pointer to a plain function converts implicitly to a pointer to
// a function type carrying `__attribute__((noreturn))`. Not a standard
// conversion ([conv.fctptr] only drops `noexcept`); GCC 16.1 accepts it
// silently, clang 19/22 reject it. kiln accepts it as GCC does and warns
// (-Wincompatible-function-pointer-types). Ranked as a function pointer
// conversion, so the identity wins `ov(fp)` over `ov(nfp)`. Found running
// GCC's C++ testsuite. WP-1551.
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 19/22 reject a conversion that adds noreturn to a function pointer
//@ strict-diags
//@ stdout: 1 1 1 1 1
//@ diag: 28:11 warning "conversion from 'void (*)(int)' to 'nfp' (aka 'void (*)(int) __attribute__((noreturn))') adds 'noreturn' to the function type (a GCC extension)"
//@ diag: 30:7 warning "conversion from 'fp' (aka 'void (*)(int)') to 'nfp' (aka 'void (*)(int) __attribute__((noreturn))') adds 'noreturn' to the function type (a GCC extension)"
//@ diag: 31:11 warning "conversion from 'void (*)(int)' to 'nfp' (aka 'void (*)(int) __attribute__((noreturn))') adds 'noreturn' to the function type (a GCC extension)"
//@ diag: 32:16 warning "conversion from 'void (*)(int)' to 'nfp' (aka 'void (*)(int) __attribute__((noreturn))') adds 'noreturn' to the function type (a GCC extension)"
//@ diag: 23:24 warning "conversion from 'fp' (aka 'void (*)(int)') to 'nfp' (aka 'void (*)(int) __attribute__((noreturn))') adds 'noreturn' to the function type (a GCC extension)"
extern "C" int printf(const char *, ...);
typedef void (*fp)(int);
typedef void (*nfp)(int) __attribute__((noreturn));
void plain(int) {}
int take(nfp p) { return p != 0; }
nfp ret(fp p) { return p; }
int ov(fp) { return 1; }
int ov(nfp);
struct S { nfp m; };
int main() {
  nfp a = plain;
  fp b = plain;
  a = b;
  S s = { plain };
  int t = take(&plain);
  printf("%d %d %d %d %d\n", ov(plain), ov(&plain), a != 0, s.m != 0, t + (ret(b) != 0) - 1);
}
