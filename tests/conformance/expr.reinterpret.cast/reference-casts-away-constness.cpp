// [expr.reinterpret.cast]/2, /11, [expr.const.cast]/8: a reinterpret_cast to
// a reference must not cast away constness, at any level of the pointer to
// the referenced type (`int *&` from a `const int *` lvalue), nor may a
// pointer cast (`int **` from `const int **`, or adding `const` below a
// level that is not `const`: `const int **` from `int **`); a prvalue binds
// no reference and a bit-field is no object a reference can name (clang
// `TryReinterpretCast`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:12 error "reinterpret_cast from 'const float' to 'float &' casts away qualifiers"
//@ diag: 22:13 error "reinterpret_cast from 'const float' to 'float &&' casts away qualifiers"
//@ diag: 24:15 error "reinterpret_cast from 'const int' to 'unsigned int &' casts away qualifiers"
//@ diag: 26:10 error "reinterpret_cast from 'volatile int' to 'int &' casts away qualifiers"
//@ diag: 27:10 error "reinterpret_cast from rvalue to reference type 'int &'"
//@ diag: 29:10 error "reinterpret_cast from bit-field lvalue to reference type 'int &'"
//@ diag: 31:11 error "reinterpret_cast from 'const int *' to 'int *&' casts away qualifiers"
//@ diag: 33:11 error "reinterpret_cast from 'const int **' to 'int **' casts away qualifiers"
//@ diag: 35:17 error "reinterpret_cast from 'int **' to 'const int **' casts away qualifiers"
//@ diag: 38:11 error "reinterpret_cast from 'const int *const *' to 'int **' casts away qualifiers"
const float x = 2.5f;
float &a = reinterpret_cast<float &>(x);
float &&b = reinterpret_cast<float &&>(x);
const int ci = 0;
unsigned &c = reinterpret_cast<unsigned &>(ci);
volatile int vi;
int &d = reinterpret_cast<int &>(vi);
int &e = reinterpret_cast<int &>(3);
struct B { int f : 3; } bb;
int &h = reinterpret_cast<int &>(bb.f);
const int *cp;
int *&i = reinterpret_cast<int *&>(cp);
const int **cpp;
int **j = reinterpret_cast<int **>(cpp);
int **pp;
const int **k = reinterpret_cast<const int **>(pp);
const int *const *l = reinterpret_cast<const int *const *>(pp);
const int *const *cpc;
int **m = reinterpret_cast<int **>(cpc);
