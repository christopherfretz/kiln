// [clang ext] A parameter may not be qualified with an address space
// (clang `CheckParameter`): checked on the declared type, so an array of
// qualified elements is rejected before it becomes a pointer, in a
// template or not, a pointer to an array of them is not. A function type
// inside a function template's declaration is checked again when it is
// substituted, failing the deduction (a parameter's or a trailing return
// type's). Reduced from real code. spec/sema.md §5.8, WP-1823.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:13 error "no matching function for call to 'm'"
//@ diag: 22:25 note "candidate template ignored: substitution failure [with T = __attribute__((address_space(1))) int]: parameter may not be qualified with an address space"
//@ diag: 25:13 error "no matching function for call to 'n'"
//@ diag: 24:23 note "candidate template ignored: substitution failure [with N = 1]: parameter may not be qualified with an address space"
//@ diag: 26:18 error "parameter may not be qualified with an address space"
//@ diag: 27:18 error "parameter may not be qualified with an address space"
//@ diag: 28:20 error "parameter may not be qualified with an address space"
//@ diag: 31:15 error "explicit instantiation of 'k' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 30:23 note "candidate template ignored: substitution failure [with N = 1]: parameter may not be qualified with an address space"
//@ diag: 33:13 error "no matching function for call to 's'"
//@ diag: 32:25 note "candidate template ignored: substitution failure [with T = __attribute__((address_space(1))) int]: parameter may not be qualified with an address space"
#define AS(n) __attribute__((address_space(n)))
template <class T> void m(void (*)(T)) {}
void um() { m<int __attribute__((address_space(1)))>(nullptr); }
template <int N> auto n() -> void (*)(int AS(N)) { return nullptr; }
void un() { n<1>(); }
void f(int AS(1) a[2]);
void g(int AS(1) a[2][3]);
void (*h)(int AS(1)[2]);
void k(int AS(1) (*a)[2]);
template <int N> void k(int AS(N) a[2]) {}
template void k<1>(int AS(1) *);
template <class T> void s(T a[2]) {}
void us() { s<int __attribute__((address_space(1)))>(nullptr); }
