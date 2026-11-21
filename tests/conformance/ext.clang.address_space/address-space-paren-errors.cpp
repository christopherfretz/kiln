//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:7 error "function type may not be qualified with an address space"
//@ diag: 18:27 note "expanded from macro 'AS'"
//@ diag: 21:15 error "function type may not be qualified with an address space"
//@ diag: 18:27 note "expanded from macro 'AS'"
//@ diag: 22:14 error "function type may not be qualified with an address space"
//@ diag: 18:27 note "expanded from macro 'AS'"
//@ diag: 23:17 error "function type may not be qualified with an address space"
//@ diag: 18:27 note "expanded from macro 'AS'"
//@ diag: 24:7 error "function type may not be qualified with an address space"
//@ diag: 18:27 note "expanded from macro 'AS'"
//@ diag: 25:8 error "an attribute list cannot appear here"
//@ diag: 26:7 error "an attribute list cannot appear here"
// A space after a grouping `(` lands on the type at the parentheses
// (WP-1812): a function type there is an error, wherever the declarator
// appears; a `[[...]]` attribute list may not follow a grouping `(`.
#define AS __attribute__((address_space(1)))

void (AS f2)();
typedef void (AS *FP)();
void g(void (AS *gp)());
using U = void (AS *)();
void (AS f3)() {}
void (([[clang::address_space(1)]] *fp5))();
void ([[gnu::noreturn]] *fp7)();
