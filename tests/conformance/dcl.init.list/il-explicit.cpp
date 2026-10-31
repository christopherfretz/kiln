// [over.match.list]/1: copy-list-initialization that chooses an explicit
// constructor is ill-formed; as an implicit conversion sequence
// ([over.ics.list]) explicit constructors are not used at all.
//@ expect: compile-fail
//@ diag: 18:3 error "chosen constructor is explicit in copy-initialization"
//@ diag: 15:12 note "explicit constructor declared here"
//@ diag: 20:12 error "no matching function for call to 'f'"
//@ diag: 19:6 note "candidate function not viable: cannot convert initializer list argument to 'X'"
//@ diag: 26:14 error "chosen constructor is explicit in copy-initialization"
//@ diag: 22:12 note "explicit constructor declared here"
//@ diag: 30:3 error "chosen constructor is explicit in copy-initialization"
//@ diag: 28:12 note "explicit constructor declared here"
#include "Inputs/initializer_list.h"
struct X {
  explicit X(std::initializer_list<int>);
};
X a{1, 2};
X b = {1, 2};
void f(X);
void g() { f({3}); }
struct Y {
  explicit Y(std::initializer_list<int>);
  Y(int, int);
};
void h(Y);
void k() { h({1, 2}); }
struct Z {
  explicit Z(int, int);
};
Z z = {1, 2};
