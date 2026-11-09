// [basic.life]/1, [expr.prim.id.dtor]/2, [expr.const]/10: an explicit
// destructor call or pseudo-destructor call ends the object's lifetime
// during constant evaluation, through a pointer (`p->~I()`), a template
// (`p->~U()`), a qualified name (`x.I::~I()`) or `~decltype(x)`; a later
// read, an assignment through a pointer, or a second destruction is not
// a constant expression (clang's `HandleDestruction`; a lockdown of the
// WP-222 lifetime marks).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:68 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 38:15 note "in call to 'ptr_read()'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 39:63 note "destruction of object outside its lifetime is not allowed in a constant expression"
//@ diag: 40:15 note "in call to 'ptr_twice()'"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 41:58 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 42:15 note "in call to 'templ_read()'"
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:58 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 44:15 note "in call to 'qual_read()'"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:69 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 46:15 note "in call to 'decltype_read()'"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:69 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 48:15 note "in call to 'class_ptr_read()'"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:60 note "destruction of object outside its lifetime is not allowed in a constant expression"
//@ diag: 50:15 note "in call to 'class_qual_twice()'"
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 51:70 note "assignment to object outside its lifetime is not allowed in a constant expression"
//@ diag: 52:15 note "in call to 'write_restarts()'"
using I = int;
struct S { int v; constexpr S(int x) : v(x) {} constexpr ~S() {} };
template <class U> constexpr void kill(U* p) { p->~U(); }
constexpr int ptr_read() { int x = 1; int* p = &x; p->~I(); return x; }
static_assert(ptr_read() == 1);
constexpr bool ptr_twice() { int x = 1; int* p = &x; p->~I(); p->~I(); return true; }
static_assert(ptr_twice());
constexpr int templ_read() { int x = 1; kill(&x); return x; }
static_assert(templ_read() == 1);
constexpr int qual_read() { int x = 1; x.I::~I(); return x; }
static_assert(qual_read() == 1);
constexpr int decltype_read() { int x = 1; x.~decltype(x)(); return x; }
static_assert(decltype_read() == 1);
constexpr int class_ptr_read() { S s(1); S* p = &s; p->~S(); return p->v; }
static_assert(class_ptr_read() == 1);
constexpr int class_qual_twice() { S s(1); s.S::~S(); s.S::~S(); return 0; }
static_assert(class_qual_twice() == 0);
constexpr int write_restarts() { int x = 1; int* p = &x; p->~I(); *p = 2; return x; }
static_assert(write_restarts() == 2);
