// [class.default.ctor]/2: a non-aggregate class with no user-declared
// constructor whose reference or const scalar member has no default
// member initializer gets clang's warning that the member will never be
// initialized (clang `CheckCompletedCXXClass`; no -W group). A
// specialization is checked when instantiated, and is named with its
// template arguments; a lambda, an aggregate and a class with an
// in-class initializer or a user-declared (even deleted or defaulted)
// constructor are not diagnosed.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 42:7 warning "class 'A' does not declare any constructor to initialize its non-modifiable members"
//@ diag: 42:21 note "const member 'c' will never be initialized"
//@ diag: 42:29 note "reference member 'r' will never be initialized"
//@ diag: 43:8 warning "struct 'B' does not declare any constructor to initialize its non-modifiable members"
//@ diag: 43:40 note "const member 'c' will never be initialized"
//@ diag: 44:8 warning "struct 'C' does not declare any constructor to initialize its non-modifiable members"
//@ diag: 44:43 note "reference member 'r' will never be initialized"
//@ diag: 48:7 warning "union 'U' does not declare any constructor to initialize its non-modifiable members"
//@ diag: 48:30 note "const member 'c' will never be initialized"
//@ diag: 46:25 warning "class 'E<int>' does not declare any constructor to initialize its non-modifiable members"
//@ diag: 52:15 note "in instantiation of template class 'E<int>' requested here"
//@ diag: 46:37 note "const member 'c' will never be initialized"
//@ diag: 46:25 warning "class 'E<long>' does not declare any constructor to initialize its non-modifiable members"
//@ diag: 52:108 note "in instantiation of template class 'E<long>' requested here"
//@ diag: 46:37 note "const member 'c' will never be initialized"
//@ diag: 54:7 warning "class 'K1' does not declare any constructor to initialize its non-modifiable members"
//@ diag: 54:39 note "const member 'p' will never be initialized"
//@ diag: 54:61 note "const member 'e' will never be initialized"
//@ diag: 54:74 note "const member 'bf' will never be initialized"
//@ diag: 55:8 warning "struct 'K2' does not declare any constructor to initialize its non-modifiable members"
//@ diag: 55:50 note "const member 'c' will never be initialized"
//@ diag: 57:38 warning "struct 'In' does not declare any constructor to initialize its non-modifiable members"
//@ diag: 58:10 note "in instantiation of member class 'K4<int>::In' requested here"
//@ diag: 57:46 note "reference member 'r' will never be initialized"
//@ diag: 59:19 warning "struct 'Loc' does not declare any constructor to initialize its non-modifiable members"
//@ diag: 59:38 note "const member 'd' will never be initialized"
//@ diag: 60:13 warning "explicitly defaulted default constructor is implicitly deleted"
//@ diag: 60:39 note "default constructor of 'K5' is implicitly deleted because field 'c' of const-qualified type 'const int' would not be initialized"
//@ diag: 60:20 note "replace 'default' with 'delete'"
//@ diag: 61:8 warning "struct 'K6' does not declare any constructor to initialize its non-modifiable members"
//@ diag: 61:23 note "const member 'c' will never be initialized"
class A { const int c; int &r; public: int x; };
struct B { virtual void f(); const int c; };
struct C { const int c = 1; private: int &r; };
struct D { const int c; };
template<class T> class E { const T c; };
E<int> *e;
union U { private: const int c; };
struct F { F() = delete; private: const int c; };
struct G { G(const G&); private: const int c; };
struct H { private: static const int c; int &r = *new int; };
static_assert(sizeof(E<int>) == 4); struct L { const int c; }; auto lam = [x = 1]{ return x; }; struct I : E<long> { int z; };
struct B0 { B0(int); };
class K1 { const int a[2]; int *const p; enum E { X } const e; const int bf : 3; };
struct K2 : private B0 { using B0::B0; const int c; };
struct K3 { struct { const int c; }; virtual ~K3(); };
template<class T> struct K4 { struct In { T &r; virtual void f(); }; };
int s1 = sizeof(K4<int>::In);
void g() { struct Loc { const double d; virtual void f() {} }; }
struct K5 { K5() = default; const int c; };
struct K6 { const int c; mutable int m; private: int x; };
