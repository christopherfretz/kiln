// The GNU conversion that adds `noreturn` to a function pointer is not a
// standard conversion, so `static_cast` still rejects it (as clang does),
// and the two pointer types stay distinct for template argument
// deduction. WP-1551.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:9 error "static_cast from 'void (*)(int)' to 'nfp' (aka 'void (*)(int) __attribute__((noreturn))') is not allowed"
//@ diag: 18:10 error "no matching function for call to 'deduce'"
//@ diag: 16:23 note "candidate template ignored: deduced conflicting types for parameter 'T' ('nfp' (aka 'void (*)(int) __attribute__((noreturn))') vs. 'fp' (aka 'void (*)(int)'))"
typedef void (*fp)(int);
typedef void (*nfp)(int) __attribute__((noreturn));
void plain(int);
nfp a;
fp b;
nfp c = static_cast<nfp>(plain);
template<class T> int deduce(T, T);
int ok = deduce(a, a);
int dd = deduce(a, b);
