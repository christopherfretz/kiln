// [class.temporary]/4,6, -Wdangling-assignment and aggregates (WP-674): a
// pointer or view assigned the address of (or a view of) a temporary dangles
// after the assignment, whatever the assigned lvalue (a local, an array
// element, a member through a pointer); an aggregate's reference member
// bound to a lifetimebound call's temporary argument and an
// initializer_list member's backing array are diagnosed in a local
// aggregate (not extended through the call) and in a mem-initializer.
//@ strict-diags
//@ std: c++23
//@ requires: clang<22
//@ expect: compile-fail
//@ diag: 40:7 warning "object backing the pointer p will be destroyed at the end of the full-expression"
//@ diag: 42:12 warning "object backing the pointer arr[0] will be destroyed at the end of the full-expression"
//@ diag: 43:11 warning "object backing the pointer vp->q will be destroyed at the end of the full-expression"
//@ diag: 48:22 warning "temporary whose address is used as value of local variable 'q' will be destroyed at the end of the full-expression"
//@ diag: 48:25 warning "temporary whose address is used as value of local variable 'q' will be destroyed at the end of the full-expression"
//@ diag: 50:36 warning "temporary bound to reference member of local variable 'aa' will be destroyed at the end of the full-expression"
//@ diag: 50:39 warning "temporary bound to reference member of local variable 'aa' will be destroyed at the end of the full-expression"
//@ diag: 52:20 warning "temporary bound to reference member of local variable 'aarr' will be destroyed at the end of the full-expression"
//@ diag: 52:23 warning "temporary bound to reference member of local variable 'aarr' will be destroyed at the end of the full-expression"
//@ diag: 58:11 error "backing array for 'std::initializer_list' member 'il' is a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 56:30 note "'std::initializer_list' member declared here"
//@ diag: 58:21 error "reference subobject of member 'a' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 57:30 note "member with reference subobject declared here"
//@ diag: 59:14 error "backing array for 'std::initializer_list' member 'il' is a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 56:30 note "'std::initializer_list' member declared here"
//@ diag: 59:27 warning "reference subobject of member 'a' binds to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 57:30 note "member with reference subobject declared here"
//@ diag: 59:29 warning "reference subobject of member 'a' binds to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 57:30 note "member with reference subobject declared here"
#include <string>
#include <string_view>
#include <initializer_list>
struct S { int *p; int a[2]; };
using U = int[2];
const int &mx(const int &a [[clang::lifetimebound]], const int &b [[clang::lifetimebound]]);
struct V { int *q; std::string_view sv; };
void f(V *vp) {
  const int *p;
  p = U{1, 2};
  int *arr[2];
  arr[0] = S{}.a;
  vp->q = S{}.a;
  std::string_view sv;
  sv = std::string("x");
  const char *c;
  c = std::string().c_str();
  const int *q = &mx(1, 2);
  std::initializer_list<int> il = {1, 2};
  struct A { const int &r; } aa{mx(1, 2)};
  struct B { std::initializer_list<int> l; } bb{{1, 2}};
  A aarr[2] = {{mx(1, 2)}, {3}};
  (void)p; (void)c; (void)q; (void)il; (void)aa; (void)bb; (void)aarr;
}
struct M {
  std::initializer_list<int> il;
  struct A { const int &r; } a;
  M() : il{1, 2}, a{1} {}
  M(int) : il{1, 2}, a{mx(1,2)} {}
};
