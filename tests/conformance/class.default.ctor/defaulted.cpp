// [class.default.ctor]/2, [dcl.fct.def.default]/5: an explicitly-defaulted
// default constructor that would be deleted (-Wdefaulted-function-deleted, or an
// error out of line); [dcl.init.general]/8: const default-initialization needs a
// user-provided default constructor or a const-default-constructible class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:3 warning "explicitly defaulted default constructor is implicitly deleted"
//@ diag: 19:8 note "default constructor of 'E' is implicitly deleted because field 'r' of reference type 'int &' would not be initialized"
//@ diag: 18:9 note "replace 'default' with 'delete'"
//@ diag: 21:3 error "call to implicitly-deleted default constructor of 'E'"
//@ diag: 18:3 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 19:8 note "default constructor of 'E' is implicitly deleted because field 'r' of reference type 'int &' would not be initialized"
//@ diag: 23:6 error "defaulting this default constructor would delete it after its first declaration"
//@ diag: 22:26 note "default constructor of 'Out' is implicitly deleted because field 'r' of reference type 'int &' would not be initialized"
//@ diag: 30:10 error "default initialization of an object of const type 'const KK' without a user-provided default constructor"
//@ diag: 32:10 error "default initialization of an object of const type 'const KC' without a user-provided default constructor"
struct E {
  E() = default;
  int &r;
};
E e;
struct Out { Out(); int &r; };
Out::Out() = default;
struct VB { virtual ~VB(); };
struct OK : virtual VB { OK() = default; };
OK ok;
struct Trivial { int i; };
Trivial t;
struct KK { int a; };
const KK kk;
struct KC { int a; KC() = default; };
const KC kc;
struct KM { int a = 0; KM() = default; };
const KM km;
struct KU { KU(); int a; };
const KU ku;
