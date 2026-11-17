// [class.union.general]/2: the default constructor activates the member of
// a nested anonymous union that has a default member initializer, so
// reading another variant member of the enclosing union reads an inactive
// member ([expr.const]/10.9). Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ diag: 11:21 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 11:25 note "read of member 'z' of union with active member"
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
struct V { union { union { int x{5}; char y; }; long long z; }; };
constexpr long long r = V{}.z;
constexpr long long read_z() { V t; return t.z; }
static_assert(read_z() == 0);
