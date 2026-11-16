// [expr.new]/20, [over.ics.rank]/4: `0` converts to the `void *` of the
// reserved placement form and to a user allocation function's `long`
// parameter; both are conversions of the same rank, so the call is
// ambiguous. A non-null integer is not a null pointer constant and picks
// the `long` form. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++17
//@ diag: 17:24 error "call to 'operator new' is ambiguous"
//@ diag: 13:7 note "candidate function"
//@ diag: 14:7 note "candidate function"
typedef decltype(sizeof 0) size_t;
void *operator new(size_t, void *) noexcept;
void *operator new(size_t, long);
struct X { int a; };
X *one_ok() { return new (1) X; }
X *zero_bad() { return new (0) X; }
