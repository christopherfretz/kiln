// [basic.lookup.elab]/1, [dcl.type.elab]/2: an elaborated-type-specifier's
// lookup ignores only non-type names, so a nearer typedef, type alias,
// class template or alias template hides an outer class of the same name and
// the reference is ill-formed; a `friend class-key name;` whose name is such a
// type name in the innermost enclosing namespace is ill-formed the same way.
// A typedef is looked through only when it names a class of its own scope.
// Found by the clang negative-test run (WP-912), fixed by WP-957.
// Valid twin: tag-hidden-by-type-name-ok.cpp.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:10 error "typedef 'A' cannot be referenced with the 'struct' specifier"
//@ diag: 38:15 note "declared here"
//@ diag: 40:23 error "typedef 'A' cannot be referenced with the 'struct' specifier"
//@ diag: 38:15 note "declared here"
//@ diag: 44:10 error "template 'A' cannot be referenced with the 'struct' specifier"
//@ diag: 43:26 note "declared here"
//@ diag: 48:20 error "type alias 'A' cannot be referenced with the 'class' specifier"
//@ diag: 47:9 note "declared here"
//@ diag: 51:26 error "typedef 'T' cannot be referenced with the 'class' specifier"
//@ diag: 50:13 note "declared here"
//@ diag: 53:27 error "type alias 'U' cannot be referenced with the 'struct' specifier"
//@ diag: 52:7 note "declared here"
//@ diag: 55:26 error "alias template 'V' cannot be referenced with the 'class' specifier"
//@ diag: 54:18 note "declared here"
//@ diag: 57:26 error "template 'W' cannot be referenced with the 'class' specifier"
//@ diag: 56:24 note "declared here"
//@ diag: 58:43 error "template 'W' cannot be referenced with the 'class' specifier"
//@ diag: 56:24 note "declared here"
//@ diag: 59:42 error "typedef 'A' cannot be referenced with the 'struct' specifier"
//@ diag: 59:32 note "declared here"
//@ diag: 60:39 error "typedef 'A' cannot be referenced with the 'struct' specifier"
//@ diag: 60:29 note "declared here"
//@ diag: 61:55 error "typedef 'K' cannot be referenced with the 'class' specifier"
//@ diag: 61:39 note "declared here"
struct A { static int foo; };
namespace i0 {
  typedef int A;
  struct A *p0;
  int test() { struct A a; return a.foo; }
}
namespace i1 {
  template <class> class A;
  struct A *p1;
}
namespace i2 {
  using A = int;
  void g() { class A *p2; }
}
typedef int T;
struct S1 { friend class T; };
using U = int;
struct S2 { friend struct U; };
template <class> using V = int;
struct S3 { friend class V; };
template <class> class W;
struct S4 { friend class W; };
template <class> struct S5 { friend class W; };
namespace n { typedef struct A A; struct A *p3; }
void g() { typedef struct A A; struct A *p4; }
namespace k { struct S6 { typedef int K; friend class K; }; }
