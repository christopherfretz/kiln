// [dcl.init.aggr]/7, [basic.life]/1: the elements of an aggregate are
// initialized in order, so an initializer that reads a later element of
// its own aggregate reads an object whose lifetime has not begun, while
// reading an earlier one is fine. This holds for constexpr, constinit
// and static constexpr variables alike. Found running GCC's C++
// testsuite (WP-1468).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:13 error "constexpr variable 'g2' must be initialized by a constant expression"
//@ diag: 25:20 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 26:13 error "constexpr variable 'g3' must be initialized by a constant expression"
//@ diag: 26:23 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 32:15 error "constexpr variable 'arr2' must be initialized by a constant expression"
//@ diag: 32:27 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 35:13 error "variable does not have a constant initializer"
//@ diag: 35:1 note "required by 'constinit' specifier here"
//@ diag: 35:20 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 38:15 error "constexpr variable 'l2' must be initialized by a constant expression"
//@ diag: 38:22 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 39:22 error "constexpr variable 's2' must be initialized by a constant expression"
//@ diag: 39:29 note "read of object outside its lifetime is not allowed in a constant expression"
struct A { int ar[3]; };
constexpr A g1 = { 0, g1.ar[0] };
constexpr A g2 = { g2.ar[0] };
constexpr A g3 = { 1, g3.ar[2] };
struct B { int x; int y = x + 1; };
constexpr B b1 = { 4 };
static_assert(b1.y == 5);
constexpr int arr[3] = { 1, arr[0] + 1, arr[1] + 1 };
static_assert(arr[2] == 3);
constexpr int arr2[3] = { arr2[1], 1, 2 };
A n1 = { 0, n1.ar[0] };
A n2 = { n2.ar[0] };
constinit A n3 = { n3.ar[1] };
int f() {
  constexpr A l1 = { 0, l1.ar[0] };
  constexpr A l2 = { l2.ar[0] };
  static constexpr A s2 = { s2.ar[0] };
  return l1.ar[0] + l2.ar[0] + s2.ar[0];
}
