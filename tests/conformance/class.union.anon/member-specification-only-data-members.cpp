// [class.union.anon]/1: the member-specification of an anonymous union
// shall only define non-static data members ([class.union.anon]/1 note:
// nested types and functions cannot be declared there), and they are
// public. clang checks the members once the union is complete
// (`BuildAnonymousStructOrUnion`) and reports each at its name: a named
// nested class, an enumeration (named, unnamed, scoped, opaque), a typedef
// and an alias-declaration are types; a member function is a function; a
// member template, a friend and a using-enum-declaration are neither data
// member; a private or protected data member is rejected; a static data
// member is rejected as it is declared. The same for an anonymous struct
// (the GNU extension), at namespace scope, at block scope, as a class
// member and in a class template (once, for the pattern). GCC 16.1 rejects
// every case too. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ diag: 36:31 error "types cannot be declared in an anonymous union"
//@ diag: 37:29 error "types cannot be declared in an anonymous union"
//@ diag: 38:24 error "types cannot be declared in an anonymous union"
//@ diag: 39:35 error "types cannot be declared in an anonymous union"
//@ diag: 40:29 error "types cannot be declared in an anonymous union"
//@ diag: 41:36 error "types cannot be declared in an anonymous union"
//@ diag: 42:30 error "types cannot be declared in an anonymous union"
//@ diag: 43:28 error "functions cannot be declared in an anonymous union"
//@ diag: 44:50 error "anonymous union can only contain non-static data members"
//@ diag: 45:37 error "anonymous union can only contain non-static data members"
//@ diag: 46:36 error "anonymous union can only contain non-static data members"
//@ diag: 47:38 error "anonymous union cannot contain a private data member"
//@ diag: 49:33 error "anonymous union cannot contain a protected data member"
//@ diag: 50:29 error "static data member 's' not allowed in anonymous union"
//@ diag: 51:26 error "types cannot be declared in an anonymous struct"
//@ diag: 52:23 error "functions cannot be declared in an anonymous struct"
//@ diag: 55:25 error "types cannot be declared in an anonymous union"
//@ diag: 58:51 error "types cannot be declared in an anonymous union"
enum Color { red };
static union { int a1; struct N { int x; } n; };
static union { int a2; enum E { e1 } en; };
static union { int a3; enum { u1 } un; };
static union { int a4; enum class SE { x }; };
static union { int a5; enum OE : int; };
static union { int a6; typedef int T; };
static union { int a7; using U = int; };
static union { int a8; int f(); };
static union { int a9; template <class T> struct X {}; };
static union { int a10; friend void ff(); };
static union { int a11; using enum Color; };
static union { int a12; private: int b12; };
struct S {
  union { int a; protected: int b; };
  union { int c; static int s; };
  struct { int d; struct M { int y; } m; };
  struct { int e; int g(); };
};
void f() {
  union { int a; struct L { int x; } l; };
  (void)a;
}
template <class T> struct TS { union { T a; using V = T; }; };
TS<int> ts;
