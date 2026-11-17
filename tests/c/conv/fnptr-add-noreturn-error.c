// C: a pointer to a plain function assigned to a pointer to a `noreturn`
// function type is clang's -Wincompatible-function-pointer-types, an
// error by default (GCC 16.1 warns -Wdiscarded-qualifiers). The other
// direction, dropping `noreturn`, is silent. WP-1551.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:5 error "incompatible function pointer types initializing 'nfp' (aka 'void (*)(int) __attribute__((noreturn))') with an expression of type 'void (int)'"
//@ diag: 22:10 error "incompatible function pointer types passing 'void (*)(int)' to parameter of type 'nfp' (aka 'void (*)(int) __attribute__((noreturn))')"
//@ diag: 17:16 note "passing argument to parameter here"
//@ diag: 24:5 error "incompatible function pointer types assigning to 'nfp' (aka 'void (*)(int) __attribute__((noreturn))') from 'fp' (aka 'void (*)(int)')"
typedef void (*fp)(int);
typedef void (*nfp)(int) __attribute__((noreturn));
void plain(int);
__attribute__((noreturn)) void dies(int);
fp ok1 = dies;
nfp ok2 = dies;
void take_n(nfp);
nfp gn = plain;
nfp a;
fp b;
void t(void) {
  take_n(&plain);
  b = a;
  a = b;
}
