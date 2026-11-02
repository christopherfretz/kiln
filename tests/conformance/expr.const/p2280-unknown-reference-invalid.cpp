// [expr.const]/10 (C++23, P2280R4): the object an unusable reference binds
// to has an unknown value and identity. Reading or modifying it, a
// virtual call on it (its dynamic type is unknown), comparing its address
// with that of another object or null, and a constant result referring to
// it are not constant expressions (clang 22's notes).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 58:41 error "static assertion expression is not an integral constant expression"
//@ diag: 58:41 note "read of variable 'x' whose value is not known"
//@ diag: 58:22 note "declared here"
//@ diag: 59:46 error "static assertion expression is not an integral constant expression"
//@ diag: 59:46 note "read of variable 's' whose value is not known"
//@ diag: 59:27 note "declared here"
//@ diag: 60:47 error "static assertion expression is not an integral constant expression"
//@ diag: 53:54 note "read of variable 's' whose value is not known"
//@ diag: 60:47 note "in call to 's.get()'"
//@ diag: 60:28 note "declared here"
//@ diag: 61:43 error "static assertion expression is not an integral constant expression"
//@ diag: 55:43 note "read of variable 'x' whose value is not known"
//@ diag: 61:43 note "in call to 'read(x)'"
//@ diag: 61:24 note "declared here"
//@ diag: 62:37 error "static assertion expression is not an integral constant expression"
//@ diag: 62:40 note "assignment to variable 'x' whose value is not known"
//@ diag: 62:18 note "declared here"
//@ diag: 63:40 error "static assertion expression is not an integral constant expression"
//@ diag: 63:41 note "increment of variable 'x' whose value is not known"
//@ diag: 63:21 note "declared here"
//@ diag: 64:47 error "static assertion expression is not an integral constant expression"
//@ diag: 64:49 note "virtual function called on object 'v' whose dynamic type is not constant"
//@ diag: 65:48 error "static assertion expression is not an integral constant expression"
//@ diag: 65:54 note "comparison between pointers to unrelated objects '&a[4]' and 'nullptr' has unspecified value"
//@ diag: 66:45 error "static assertion expression is not an integral constant expression"
//@ diag: 66:48 note "comparison between pointers to unrelated objects '&x' and '&g' has unspecified value"
//@ diag: 67:52 error "static assertion expression is not an integral constant expression"
//@ diag: 67:55 note "comparison between pointers to unrelated objects '&x' and '&y' has unspecified value"
//@ diag: 68:48 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 68:48 note "reference to 'x' is not a constant expression"
//@ diag: 68:28 note "declared here"
//@ diag: 69:46 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 69:46 note "pointer to 'x' is not a constant expression"
//@ diag: 69:26 note "declared here"
//@ diag: 71:16 error "constexpr variable 'r2' must be initialized by a constant expression"
//@ diag: 71:16 note "initializer of 'er' is not a constant expression"
//@ diag: 70:13 note "declared here"
//@ diag: 73:44 error "static assertion expression is not an integral constant expression"
//@ diag: 73:44 note "function parameter 'p' with unknown value cannot be used in a constant expression"
//@ diag: 73:25 note "declared here"
//@ diag: 75:57 error "static assertion expression is not an integral constant expression"
//@ diag: 76:15 note "in instantiation of function template specialization 'dependent<int>' requested here"
//@ diag: 75:57 note "read of variable 'x' whose value is not known"
//@ diag: 75:38 note "declared here"
struct S { int m; constexpr int get() const { return m; } };
struct V { virtual constexpr int f() const { return 1; } };
constexpr int read(const int &r) { return r; }
int g;

void read_param(int &x) { static_assert(x == 1); }
void read_member(const S &s) { static_assert(s.m == 0); }
void read_in_call(const S &s) { static_assert(s.get() == 0); }
void read_through(int &x) { static_assert(read(x) == 0); }
void assign(int &x) { static_assert((x = 1, true)); }
void increment(int &x) { static_assert((++x, true)); }
void virtual_call(const V &v) { static_assert(v.f() == 1); }
void compare_null(int (&a)[4]) { static_assert(&a[4] != nullptr); }
void compare_global(int &x) { static_assert(&x != &g); }
void compare_other(int &x, int &y) { static_assert(&x != &y); }
void reference_result(int &x) { constexpr int &r = x; }
void pointer_result(int &x) { constexpr int *p = &x; }
extern int &er;
constexpr int &r2 = er;
// A pointer parameter is not a reference: its value is still unknown.
void pointer_param(int *p) { static_assert(p != nullptr); }

template <class T> void dependent(T &x) { static_assert(x == 0); }
template void dependent<int>(int &);
