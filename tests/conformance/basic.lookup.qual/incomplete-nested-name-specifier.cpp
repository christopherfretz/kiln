// [expr.prim.id.qual]/1, [basic.lookup.qual.general]/1: a class or enumeration
// named by a nested-name-specifier must be complete before a name is looked up
// in it: "incomplete type 'F' named in nested name specifier" at the lookup,
// with the forward-declaration note for a class (none for an opaque enum, an
// undefined class template specialization says "implicit instantiation of
// undefined template"). The qualified name is invalid and declares nothing.
// clang prints the group twice where it parses the name tentatively first.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 50:1 error "incomplete type 'F' named in nested name specifier" dup(clang)
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 52:1 error "incomplete type 'E' named in nested name specifier" dup(clang)
//@ diag: 54:1 error "incomplete type 'E2' named in nested name specifier" dup(clang)
//@ diag: 56:1 error "implicit instantiation of undefined template 'G<int>'" dup(clang)
//@ diag: 55:24 note "template is declared here"
//@ diag: 57:47 error "incomplete type 'F' named in nested name specifier" dup(clang)
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 58:25 error "incomplete type 'F' named in nested name specifier"
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 58:46 error "incomplete type 'F' named in nested name specifier"
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 59:16 error "incomplete type 'F' named in nested name specifier"
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 60:12 error "incomplete type 'F' named in nested name specifier"
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 61:9 error "incomplete type 'F' named in nested name specifier"
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 62:11 error "incomplete type 'F' named in nested name specifier" dup(clang)
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 63:9 error "incomplete type 'F' named in nested name specifier" dup(clang)
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 64:20 error "incomplete type 'F' named in nested name specifier" dup(clang)
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 65:1 error "incomplete type 'F' named in nested name specifier" dup(clang)
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 67:6 error "incomplete type 'Inc' named in nested name specifier"
//@ diag: 66:8 note "forward declaration of 'Inc'"
//@ diag: 68:10 error "use of undeclared identifier 'k'"
//@ diag: 69:8 error "incomplete type 'F' named in nested name specifier"
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 70:7 error "forward declaration of class cannot have a nested name specifier"
//@ diag: 70:7 error "incomplete type 'F' named in nested name specifier"
//@ diag: 49:8 note "forward declaration of 'F'"
//@ diag: 71:27 error "incomplete type 'Outer::In' named in nested name specifier" dup(clang)
//@ diag: 71:23 note "forward declaration of 'Outer::In'"
//@ diag: 73:1 error "incomplete type 'Fwd' named in nested name specifier" dup(clang)
//@ diag: 72:16 note "forward declaration of 'Fwd'"
struct F;
F::x y;
enum class E;
E::x w;
enum E2 : int;
E2::x w2;
template<class> struct G;
G<int>::x z;
template<class T> struct D { typename T::x a; F::x b; };
struct Fr { friend void F::f(); friend class F::G; };
int s = sizeof(F::x);
void f() { F::x(); }
int g = F::v;
using U = F::u;
typedef F::t V;
template<class T = F::z> struct Q;
F::p *p;
struct Inc;
void Inc::k() {}
int kk = k();
struct F::S {};
class F::H;
struct Outer { struct In; In::x m; };
typedef struct Fwd TF;
TF::z tz;

// Not diagnosed: a pointer to member, a dependent qualifier (also in a
// friend declaration), the class or enumeration being defined.
int F::*pm;
template<class T> void tf() { T::x(); }
template<class T> struct DF { friend class T::G; };
struct Def { void f() { Def::g(); } static void g(); };
enum E3 { e1, e2 = E3::e1 };
