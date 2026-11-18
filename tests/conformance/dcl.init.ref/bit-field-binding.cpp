// [dcl.init.ref]/5: a bit-field is not referenceable. A non-const lvalue
// reference cannot bind one (/5.1, /5.2 do not apply); an rvalue reference
// binds a temporary copy of a bit-field xvalue or prvalue (/5.4.2) but not
// of a bit-field lvalue (/5.4.4: "the initializer expression shall not be
// an lvalue"; kiln accepted it before WP-1692), also through a glvalue
// conditional that is a bit-field ([expr.cond]/5), a comma, or a vector
// element; `&` of one is ill-formed ([expr.unary.op]/3). clang 22's
// diagnostics; GCC 16.1 rejects the same lines.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:7 error "rvalue reference to type 'int' cannot bind to lvalue of type 'int'"
//@ diag: 32:7 error "rvalue reference to type 'int' cannot bind to lvalue of type 'int'"
//@ diag: 33:7 error "rvalue reference to type 'int' cannot bind to lvalue of type 'int'"
//@ diag: 34:7 error "rvalue reference to type 'int' cannot bind to lvalue of type 'int'"
//@ diag: 35:13 error "rvalue reference to type 'const int' cannot bind to lvalue of type 'int'"
//@ diag: 36:7 error "rvalue reference to type 'int' cannot bind to lvalue of type 'int'"
//@ diag: 37:6 error "non-const reference cannot bind to bit-field"
//@ diag: 38:6 error "non-const reference cannot bind to bit-field 'p'"
//@ diag: 23:16 note "bit-field is declared here"
//@ diag: 39:11 error "address of bit-field requested"
//@ diag: 41:12 error "no matching function for call to 'f'"
//@ diag: 40:6 note "candidate function not viable: expects an rvalue for 1st argument"
struct S { int p : 4; int q : 12; int w; } s;
struct M { int p : 4; };
M mk();
bool c;
int g;
typedef int v4 __attribute__((vector_size(16)));
v4 vv;

int &&r1 = s.p;
int &&r2 = c ? s.p : s.q;
int &&r3 = c ? s.p : s.w;
int &&r4 = (g = 1, s.p);
const int &&r5 = s.p;
int &&r6 = vv[1];
int &l1 = c ? s.p : s.q;
int &l2 = (g = 2, s.p);
int *a1 = &(c ? s.p : s.q);
void f(int &&);
void h() { f(s.p); }

// Valid: a bit-field xvalue or prvalue binds a temporary copy, and so does
// a const lvalue reference.
int &&x1 = static_cast<M &&>(*(M *)0).p;
int &&x2 = mk().p;
const int &x3 = c ? s.p : s.q;
const int &x4 = c ? s.p : s.w;
void k() { f(mk().p); }
