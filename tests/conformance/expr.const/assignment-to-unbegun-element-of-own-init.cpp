// [expr.const]/10.8, [basic.life]/1, /7, [dcl.init.aggr]/7: the elements
// of an array (and the members of a class) under initialization are
// initialized in order, so an assignment in an earlier element's
// initializer to a later element writes an object whose lifetime has not
// begun: undefined, so not a constant expression. Kiln accepted it and
// then dropped the store (found running GCC's C++ testsuite).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 15:47 note "assignment to object outside its lifetime is not allowed in a constant expression"
//@ diag: 18:13 error "constexpr variable 's' must be initialized by a constant expression"
//@ diag: 18:42 note "assignment to object outside its lifetime is not allowed in a constant expression"
// An element already initialized may be assigned: `b` is a constant.
constexpr int a[4] = {const_cast<int &>(a[3]) = 5, 1};
constinit int b[2] = {1, b[0] = 2};
struct S { int x[3]; int y; };
constexpr S s = {{const_cast<int &>(s.y) = 7, 2}};
int main() { return b[0] == 2 && b[1] == 2 ? 0 : 1; }
