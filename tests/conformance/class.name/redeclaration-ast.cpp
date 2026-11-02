// [class.name]/2: `struct N;` with N already declared in the scope
// redeclares it, so each `struct N;` is a CXXRecordDecl of N's chain (clang:
// `prev`) and the definition attaches to it. `N *` marks only the
// declaration lookup finds (the latest) referenced; an
// elaborated-type-specifier (`struct N *a`) marks none ([dcl.type.elab]/2,
// clang `ActOnTag`). Member and local classes redeclare the same way.
//@ expect: ast
struct N;
struct N;
struct N { int x; };
struct N;
N *p;
struct N *a;
void f() { struct L; struct L; struct L {}; L *l; }
template<class T> struct TT { struct I; struct I {}; };
TT<int> tt;
