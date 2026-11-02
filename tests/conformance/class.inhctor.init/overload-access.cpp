// [over.match.funcs]/8: an inherited constructor whose first parameter is a
// reference to a class reference-related to the derived class is not a
// candidate for a single argument (copy/move constructors are not listed);
// [class.inhctor.init]/1, [namespace.udecl]/19: an inherited constructor
// has the access of the base constructor; [namespace.udecl]/3: the
// nested-name-specifier names a direct base; [over.match.best]/2.7: a
// constructor of the derived class beats an inherited one with the same
// parameter types.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:3 error "no matching constructor for initialization of 'B'"
//@ diag: 28:12 note "candidate inherited constructor not viable: no known conversion from 'A' to 'int' for 1st argument"
//@ diag: 29:25 note "constructor from base class 'A' inherited here"
//@ diag: 28:42 note "candidate inherited constructor not viable: no known conversion from 'A' to 'char' for 1st argument"
//@ diag: 29:25 note "constructor from base class 'A' inherited here"
//@ diag: 28:62 note "candidate inherited constructor not viable: no known conversion from 'A' to 'double' for 1st argument"
//@ diag: 29:25 note "constructor from base class 'A' inherited here"
//@ diag: 29:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'A' to 'const B' for 1st argument"
//@ diag: 29:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'A' to 'B' for 1st argument"
//@ diag: 29:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
//@ diag: 32:3 error "calling a private constructor of class 'A'"
//@ diag: 28:42 note "declared private here"
//@ diag: 33:3 error "calling a protected constructor of class 'A'"
//@ diag: 28:62 note "declared protected here"
//@ diag: 36:22 error "', which is not a base class of 'D'"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
struct A { A(int); A(const A&); private: A(char); protected: A(double); };
struct B : A { using A::A; };
A a(1);
B b1(a);
B b2('c');
B b3(1.0);
B b4(1);
struct C { C(int); };
struct D : A { using C::C; };
struct E { E(int, int = 0); };
struct F : E { using E::E; F(int); };
F f(1);
struct G : A { using A::A; G(int); };
G g(1);
