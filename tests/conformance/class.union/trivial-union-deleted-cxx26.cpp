// [class.dtor]/7.2 (P3074R7, with CWG3041 as proposed): a union's
// defaulted destructor is deleted when a variant member of class type M has
// a deleted, inaccessible or non-trivial destructor and either the member
// has a default member initializer and M's destructor is non-trivial, or
// default-initializing the union would select a user-provided (or deleted)
// constructor. kiln follows CWG3041 rather than the DIS text, which deletes
// the destructor of every union whose default constructor is user-provided
// (spec/sema-class.md §4.2). [class.default.ctor]/2.4: a variant member
// with a default member initializer whose destructor is deleted makes the
// default constructor deleted.
//@ skip(clang): clang 22 does not implement P3074 (trivial unions)
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:3 error "attempt to use a deleted function"
//@ diag: 27:13 note "destructor of 'W' is implicitly deleted because variant field 's' has a non-trivial destructor"
//@ diag: 30:3 error "attempt to use a deleted function"
//@ diag: 29:20 note "destructor of 'X' is implicitly deleted because variant field 's' has a non-trivial destructor"
//@ diag: 39:3 error "call to implicitly-deleted default constructor of 'H'"
//@ diag: 38:13 note "default constructor of 'H' is implicitly deleted because field 'g' has a deleted destructor"
//@ diag: 37:12 note "'~G' has been explicitly marked deleted here"
struct S {
  int *p;
  constexpr S(int v) : p(new int(v)) {}
  constexpr ~S() { delete p; }
};
union W { S s = 1; int i; };
W w;
union X { X() {} S s; };
X x;
// Fine: no initializer, defaulted default constructor.
union Y { S s; int i; };
Y y;
// Fine: the variant member with the initializer has a trivial destructor.
union Z { int i = 0; S s; };
Z z;
struct G { ~G() = delete; };
union H { G g{}; };
H h;
