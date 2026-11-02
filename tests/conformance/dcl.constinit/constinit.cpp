// [dcl.constinit]: only a variable with static or thread storage duration may
// be constinit, and the initializing declaration of one must have a constant
// initializer ([expr.const]; destruction is not checked). The specifier on an
// earlier declaration applies, and clang warns that the initializing one lacks
// it (-Wmissing-constinit, on by default), also in an instantiation.
// Pinned to C++23: P3533R2 (C++26) allows virtual bases in constant evaluation.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 53:13 error "variable does not have a constant initializer"
//@ diag: 53:1 note "required by 'constinit' specifier here"
//@ diag: 53:13 note "constructor inherited from base class 'V' cannot be used in a constant expression; derived class cannot be implicitly initialized"
//@ diag: 52:33 note "declared here"
//@ diag: 56:13 error "variable does not have a constant initializer"
//@ diag: 56:1 note "required by 'constinit' specifier here"
//@ diag: 56:16 note "non-constexpr constructor 'NL' cannot be used in a constant expression"
//@ diag: 54:13 note "declared here"
//@ diag: 57:12 error "local variable cannot be declared 'constinit'"
//@ diag: 59:15 error "variable does not have a constant initializer"
//@ diag: 59:1 note "required by 'constinit' specifier here"
//@ diag: 59:19 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 58:5 note "declared here"
//@ diag: 61:28 error "variable does not have a constant initializer"
//@ diag: 61:1 note "required by 'constinit' specifier here"
//@ diag: 61:32 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 58:5 note "declared here"
//@ diag: 63:8 warning "'constinit' specifier missing on initializing declaration of 'm'"
//@ diag: 62:19 note "variable declared constinit here"
//@ diag: 63:8 error "variable does not have a constant initializer"
//@ diag: 62:19 note "required by 'constinit' specifier here"
//@ diag: 63:12 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 58:5 note "declared here"
//@ diag: 64:7 warning "'constinit' specifier missing on initializing declaration of 'n'"
//@ diag: 62:43 note "variable declared constinit here"
//@ diag: 64:7 error "variable does not have a constant initializer"
//@ diag: 62:43 note "required by 'constinit' specifier here"
//@ diag: 64:7 note "non-constexpr constructor 'NL' cannot be used in a constant expression"
//@ diag: 54:13 note "declared here"
//@ diag: 65:8 warning "'constinit' specifier missing on initializing declaration of 'k'"
//@ diag: 62:66 note "variable declared constinit here"
//@ diag: 69:15 error "variable does not have a constant initializer"
//@ diag: 69:1 note "required by 'constinit' specifier here"
//@ diag: 69:22 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 78:30 warning "'constinit' specifier missing on initializing declaration of 'q'"
//@ diag: 77:38 note "variable declared constinit here"
//@ diag: 78:30 error "variable does not have a constant initializer"
//@ diag: 79:18 note "in instantiation of static data member 'TT<A>::q' requested here"
//@ diag: 77:38 note "required by 'constinit' specifier here"
//@ diag: 78:34 note "non-constexpr function 'h' cannot be used in a constant expression"
//@ diag: 76:5 note "declared here"
struct V { constexpr V(int v) : v(v) {} int v; };
struct M : virtual V { using V::V; };
constinit M m(1);
struct NL { NL(); };
struct X { int a; NL n; };
constinit X cx{};
void f() { constinit int x = 1; static constinit int sx = 1; constinit thread_local int tx = 2; }
int g();
constinit int y = g();
constinit int z;
constinit thread_local int t = g();
struct S { static constinit int m; static constinit NL n; static constinit int k; };
int S::m = g();
NL S::n;
int S::k = 3;
extern int w;
constinit int w = 2;
extern int u; int u = g();
constinit int self = *&self + 1;
struct D { constexpr D() {} ~D() {} };
constinit D d;
constexpr int sq(int i) { return i * i; }
constinit int c9 = sq(3);
constinit const char* str = "constant";
struct A {};
int h(A);
template<class T> struct TT { static constinit int q; };
template<class T> int TT<T>::q = h(T());
int use = TT<A>::q;
