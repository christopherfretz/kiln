// A temporary created in a conditional branch at an address computed in
// that branch (here an array member of a braced temporary) saves the
// address to a `cond-cleanup.save` slot so its EH cleanup can reload it
// (clang `DominatingValue<Address>::save`).
struct T { T(); T(int); ~T(); int v() const; };
int g(int);
struct A { T a[2]; int x; };
void f1(int c) { g(c ? A{{T(1), T(2)}, g(0)}.x : 0); }
struct B { T t; int x; };
void f2(int c) { g(c ? B{T(1), g(0)}.x : B{T(2), g(1)}.x); }
