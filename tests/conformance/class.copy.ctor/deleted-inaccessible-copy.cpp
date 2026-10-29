// [dcl.init.general]/16.6.2, [class.copy.ctor]/1, [dcl.fct.def.delete]/2,
// [class.access]/1, [expr.static.cast]/4, [expr.cast]/4, [expr.type.conv]/1:
// copying an object through a deleted or inaccessible copy constructor, in a
// copy- or direct-initialization, an argument, an explicit conversion and a
// return statement. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 43:5 error "call to deleted constructor of 'N'"
//@ diag: 36:17 note "'N' has been explicitly marked deleted here"
//@ diag: 44:5 error "call to deleted constructor of 'N'"
//@ diag: 36:17 note "'N' has been explicitly marked deleted here"
//@ diag: 45:9 error "calling a private constructor of class 'P'"
//@ diag: 37:11 note "implicitly declared private here"
//@ diag: 46:5 error "calling a private constructor of class 'P'"
//@ diag: 37:11 note "implicitly declared private here"
//@ diag: 47:8 error "call to deleted constructor of 'N'"
//@ diag: 36:17 note "'N' has been explicitly marked deleted here"
//@ diag: 40:13 note "passing argument to parameter 'n' here"
//@ diag: 48:9 error "calling a private constructor of class 'P'"
//@ diag: 37:11 note "implicitly declared private here"
//@ diag: 49:5 error "call to implicitly-deleted copy constructor of 'L'"
//@ diag: 39:19 note "copy constructor of 'L' is implicitly deleted because field 'k' has a deleted copy constructor"
//@ diag: 38:17 note "'K' has been explicitly marked deleted here"
//@ diag: 50:9 error "functional-style cast from 'N' to 'N' uses deleted function"
//@ diag: 36:17 note "candidate constructor has been explicitly deleted"
//@ diag: 51:9 error "calling a private constructor of class 'P'"
//@ diag: 37:11 note "implicitly declared private here"
//@ diag: 52:9 error "C-style cast from 'N' to 'N' uses deleted function"
//@ diag: 36:17 note "candidate constructor has been explicitly deleted"
//@ diag: 53:9 error "functional-style cast from 'L' to 'L' uses deleted function"
//@ diag: 39:8 note "candidate constructor (the implicit copy constructor) has been implicitly deleted"
//@ diag: 55:16 error "call to deleted constructor of 'N'"
//@ diag: 36:17 note "'N' has been explicitly marked deleted here"
//@ diag: 56:16 error "calling a private constructor of class 'P'"
//@ diag: 37:11 note "implicitly declared private here"
struct N { N(); N(const N&) = delete; };
class P { P(const P&); public: P(); };
struct K { K(); K(const K&) = delete; };
struct L { L(); K k; };
void take(N n); void takep(P p);
N n0; P p0; L l0;
void f() {
  N a = n0;
  N b(n0);
  P c = p0;
  P d(p0);
  take(n0);
  takep(p0);
  L e = l0;
  (void)N(n0);
  (void)static_cast<P>(p0);
  (void)(N)n0;
  (void)L(l0);
}
N g() { return n0; }
P h() { return p0; }
