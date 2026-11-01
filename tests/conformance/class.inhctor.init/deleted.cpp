// [class.inhctor.init]/1, [class.default.ctor]/2, [namespace.udecl]/13: an
// inheriting constructor is deleted when the inherited constructor is
// deleted or when a defaulted default constructor of the derived class
// would be deleted for one of the other subobjects (a deleted or
// inaccessible default constructor or destructor, an uninitialized
// reference or const member); the use is diagnosed as clang's
// `err_deleted_inherited_ctor_use` with the deletion notes, and the
// class is not constructible from the inherited parameters.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 51:6 error "constructor inherited by 'D1' from base class 'B' is implicitly deleted"
//@ diag: 38:33 note "constructor inherited by 'D1' is implicitly deleted because field 'x' has a deleted corresponding constructor"
//@ diag: 33:14 note "'NDC' has been explicitly marked deleted here"
//@ diag: 52:6 error "constructor inherited by 'D2' from base class 'B' is implicitly deleted"
//@ diag: 39:33 note "constructor inherited by 'D2' is implicitly deleted because field 'x' has an inaccessible default constructor"
//@ diag: 53:15 error "constructor inherited by 'D3' from base class 'B' is implicitly deleted"
//@ diag: 40:32 note "constructor inherited by 'D3' is implicitly deleted because field 'x' has a deleted destructor"
//@ diag: 35:13 note "'~DD' has been explicitly marked deleted here"
//@ diag: 54:15 error "constructor inherited by 'D4' from base class 'B' is implicitly deleted"
//@ diag: 41:32 note "constructor inherited by 'D4' is implicitly deleted because field 'x' has an inaccessible destructor"
//@ diag: 55:6 error "constructor inherited by 'D5' from base class 'B' is implicitly deleted"
//@ diag: 42:16 note "constructor inherited by 'D5' is implicitly deleted because base class 'NDC' has a deleted corresponding constructor"
//@ diag: 33:14 note "'NDC' has been explicitly marked deleted here"
//@ diag: 56:6 error "constructor inherited by 'D6' from base class 'B' is implicitly deleted"
//@ diag: 43:46 note "constructor inherited by 'D6' is implicitly deleted because field 'r' of reference type 'int &' would not be initialized"
//@ diag: 57:6 error "constructor inherited by 'D7' from base class 'B' is implicitly deleted"
//@ diag: 44:51 note "constructor inherited by 'D7' is implicitly deleted because field 'c' of const-qualified type 'const int' would not be initialized"
//@ diag: 58:6 error "constructor inherited by 'D8' from base class 'B' is implicitly deleted"
//@ diag: 45:53 note "constructor inherited by 'D8' is implicitly deleted because field 's' has a deleted corresponding constructor"
//@ diag: 45:48 note "is implicitly deleted because field 'z' of const-qualified type 'const int' would not be initialized"
//@ diag: 60:6 error "call to deleted constructor of 'D9'"
//@ diag: 37:20 note "'B' has been explicitly marked deleted here"
struct NDC { NDC() = delete; NDC(int); };
struct PDC { private: PDC(); public: PDC(int); };
struct DD { ~DD() = delete; };
struct PD { private: ~PD(); };
struct B { B(int); B(char) = delete; };
struct D1 : B { using B::B; NDC x; };
struct D2 : B { using B::B; PDC x; };
struct D3 : B { using B::B; DD x; };
struct D4 : B { using B::B; PD x; };
struct D5 : B, NDC { using B::B; };
struct D6 : B { using B::B; D6(void *); int &r; };
struct D7 : B { using B::B; D7(void *); const int c; };
struct D8 : B { using B::B; struct { const int z; } s; };
struct D9 : B { using B::B; NDC x = 1; union { int a; const int b; }; };
static_assert(!__is_constructible(D1, int));
static_assert(!__is_constructible(D5, int));
static_assert(__is_constructible(D9, int));
void f() {
  D1 a(1);
  D2 b(1);
  D3 *c = new D3(1);
  D4 *d = new D4(1);
  D5 e(1);
  D6 g(1);
  D7 h(1);
  D8 i(1);
  D9 j(1);
  D9 k('c');
}
