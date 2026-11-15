// [dcl.typedef]/5-6, [class.name]/2: a typedef-name may redeclare a class
// or enumeration of its scope when it denotes that same type, in namespace,
// class (CWG424) and block scope, by typedef or alias-declaration, also
// through a using-declaration naming the class. Inside a template a member
// class or local class is dependent, so a typedef of its name is not
// checked at the definition, nor at instantiation (clang
// `isIncompatibleTypedef`). Invalid forms: typedef-after-class-same-name.cpp.
// WP-1435.
//@ expect: compile-ok
//@ strict-diags
struct S { int a; };
typedef struct S S;
typedef S S;
using S = S;
typedef const S CS;
struct W {};
typedef W W2;
typedef W2 W;
enum E { E0 };
typedef E E;
enum class EC { X };
using EC = EC;
typedef struct T { int b; } T;
struct F;
typedef F F;
struct F { int c; };
struct H {
  struct A { int d; };
  typedef A A;
  enum K { K0 };
  using K = K;
};
namespace X { struct Y {}; }
using X::Y;
typedef X::Y Y;
void block() {
  struct L { int e; };
  typedef L L;
  L l;
  l.e = 0;
}
template <class U> struct Tm { struct t1 {}; typedef U t1; };
Tm<int> tm;
template <class U> struct Tn { struct t1 {}; typedef int t1; };
Tn<int> tn;
template <class U> void g() { struct L {}; typedef U L; }
template void g<int>();
int use(S s, T t, F f, H::A a, H::K k) { return s.a + t.b + f.c + a.d + k; }
