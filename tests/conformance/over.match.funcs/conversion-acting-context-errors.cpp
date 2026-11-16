// [over.match.funcs.general]/5, [class.access.general]/4: an inherited
// conversion function and the class's own are ranked alike; when the
// selected one is private, the conversion is an access error (it is not
// skipped for a worse accessible one); two equally good ones are
// ambiguous. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:29 error "'operator bool' is a private member of 'Y'"
//@ diag: 16:25 note "declared private here"
//@ diag: 18:26 error "'operator bool' is a private member of 'Y'"
//@ diag: 16:25 note "declared private here"
//@ diag: 21:29 error "conversion from 'const B' to 'bool' is ambiguous"
//@ diag: 20:16 note "candidate function"
//@ diag: 19:12 note "candidate function"
struct X { operator int() const { return 1; } };
struct Y : X { private: operator bool() const { return false; } };
bool f(const Y &y) { return bool(y); }
bool g(const Y &y) { if (y) return true; return false; }
struct A { operator int() const { return 1; } };
struct B : A { operator long() const { return 2; } };
long h(const B &b) { return b ? 1 : 0; }
