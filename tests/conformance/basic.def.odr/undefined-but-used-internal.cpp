// [basic.def.odr]/11: an odr-used function or variable that is not
// externally visible (internal linkage: `static`, an unnamed namespace, a
// member of a class of one or of a local class; or a specialization
// taking internal linkage from a template argument) and never defined is
// -Wundefined-internal; an odr-used undefined inline function is
// -Wundefined-inline. Both are diagnosed at the end of the translation
// unit at the first declaration, in the order of the first uses, with a
// note at the first use. Matches clang 19 and 22.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 44:13 warning "function 's1' has internal linkage but is not defined"
//@ diag: 50:13 note "used here"
//@ diag: 46:13 warning "inline function 'i1' is not defined"
//@ diag: 50:19 note "used here"
//@ diag: 40:6 warning "function '(anonymous namespace)::k' has internal linkage but is not defined"
//@ diag: 50:31 note "used here"
//@ diag: 41:12 warning "variable '(anonymous namespace)::ev' has internal linkage but is not defined"
//@ diag: 50:45 note "used here"
//@ diag: 39:36 warning "function '(anonymous namespace)::C::~C' has internal linkage but is not defined"
//@ diag: 51:15 note "used here"
//@ diag: 39:17 warning "function '(anonymous namespace)::C::f' has internal linkage but is not defined"
//@ diag: 51:20 note "used here"
//@ diag: 39:33 warning "variable '(anonymous namespace)::C::s' has internal linkage but is not defined"
//@ diag: 51:34 note "used here"
//@ diag: 48:25 warning "function 'g3<(anonymous namespace)::A>' has internal linkage but is not defined"
//@ diag: 51:37 note "used here"
//@ diag: 49:44 warning "function 'TC<(anonymous namespace)::A>::sf' has internal linkage but is not defined"
//@ diag: 51:53 note "used here"
//@ diag: 52:29 warning "function 'h3()::L::lf' has internal linkage but is not defined"
//@ diag: 52:42 note "used here"
//@ diag: 42:17 warning "function '(anonymous namespace)::O::operator==' has internal linkage but is not defined"
//@ diag: 53:25 note "used here"
//@ diag: 45:13 warning "function 's2' has internal linkage but is not defined"
//@ diag: 54:22 note "used here"
//@ diag: 47:15 warning "inline function 'c1' is not defined"
//@ diag: 54:42 note "used here"
namespace {
struct A {};
struct C { void f(); static int s; ~C(); };
void k(A);
extern int ev;
struct O { bool operator==(const O &) const; };
}
static void s1();
static void s2();
inline void i1();
constexpr int c1();
template <class T> void g3();
template <class T> struct TC { static void sf(); };
void h1() { s1(); i1(); s1(); k(A{}); (void)ev; }
void h2() { C c; c.f(); (void)C::s; g3<A>(); TC<A>::sf(); }
void h3() { struct L { void lf(); }; L().lf(); }
bool h4(O a) { return a == a; }
int h5() { auto p = &s2; (void)p; return c1(); }
