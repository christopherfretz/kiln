// [class.copy.ctor]/10, [class.copy.assign]/7, [class.default.ctor]/2,
// [module.interface], spec/modules.md §11.11 "Implicit members"
// (WP-1292): an importer's errors against implicit special members built
// from the declaration image are the ones against the replayed source.
//@ expect: compile-fail
//@ std: c++20
//@ module: imgim=Inputs/img-implicit.cppm
//@ diag: 15:18 error "call to implicitly-deleted copy constructor of 'X'"
//@ diag: 16:24 error "copy assignment operator is implicitly deleted"
//@ diag: 18:14 error "call to implicitly-deleted default constructor of 'E'"
//@ diag: 20:14 error "call to implicitly-deleted default constructor of 'R'"
import imgim;
using namespace im;
// A member's user-declared move, a const member, deleted and inaccessible destructors.
void f(X &a) { X b = a; }
void g(U &a, U &b) { a = b; }
struct E { DelD d; };
void h() { E e; }
struct R { P p; };
void m() { R r; }
