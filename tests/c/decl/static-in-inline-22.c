// C17 6.7.4p3 / C2y: a use of a `static` object or function in an inline
// function with external linkage. clang 19 says (see the -19 test) it "is used"; clang 22
// calls it a C2y extension (WP-1312, WP-1309 follow-up).
//@ requires: clang>=22
//@ args: -pedantic
//@ strict-diags
//@ diag: 15:29 warning "using static variable 's' in an inline function with external linkage is a C2y extension"
//@ diag: 15:1 note "use 'static' to give inline function 'h' internal linkage"
//@ diag: 13:12 note "'s' declared here"
//@ diag: 15:33 warning "using static function 'sf' in an inline function with external linkage is a C2y extension"
//@ diag: 15:1 note "use 'static' to give inline function 'h' internal linkage"
//@ diag: 14:12 note "'sf' declared here"
static int s;
static int sf(void) { return 0; }
inline int h(void) { return s + sf(); }
