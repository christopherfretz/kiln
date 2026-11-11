// C23 6.7.5p3 (function specifiers): an inline definition of a function
// with external linkage shall not contain a reference to an identifier
// with internal linkage. clang `-Wstatic-in-inline` (C only), pedantic
// when the use is in the main file; the "use 'static'" note only when
// the function has no storage class written. A `static inline` function
// and a block-scope `static` object (no linkage) are fine.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wstatic-in-inline
//@ diag: 26:29 warning "static variable 's' is used in an inline function with external linkage"
//@ diag: 26:1 note "use 'static' to give inline function 'f' internal linkage"
//@ diag: 23:12 note "'s' declared here"
//@ diag: 26:33 warning "static function 'sf' is used in an inline function with external linkage"
//@ diag: 26:1 note "use 'static' to give inline function 'f' internal linkage"
//@ diag: 24:12 note "'sf' declared here"
//@ diag: 26:40 warning "static function 'si' is used in an inline function with external linkage"
//@ diag: 26:1 note "use 'static' to give inline function 'f' internal linkage"
//@ diag: 25:19 note "'si' declared here"
//@ diag: 27:36 warning "static variable 's' is used in an inline function with external linkage"
//@ diag: 23:12 note "'s' declared here"
//@ diag: 29:22 warning "non-constant static local variable in inline function may be different in different files"
//@ diag: 29:1 note "use 'static' to give inline function 'k' internal linkage"
static int s;
static int sf(void) { return 1; }
static inline int si(void) { return 2; }
inline int f(void) { return s + sf() + si(); }
extern inline int g(void) { return s; }
static inline int h(void) { return s + sf(); }
inline int k(void) { static int local; return local++; }
int ok(void) { return s + sf(); }
int main(void) { return f() + g() + h() + k() + ok(); }
