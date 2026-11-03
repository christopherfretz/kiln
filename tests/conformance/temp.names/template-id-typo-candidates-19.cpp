// [temp.names]/3: a template-id whose name ordinary lookup does not find
// (or finds only functions for), not called, names nothing. clang 19
// delays its typo correction to the full expression (spec/sema.md
// "Undeclared template-ids"): a value is tried with the template arguments
// dropped; a function or template as an operand is never valid, a type or
// namespace is no candidate, and a keyword alone at the least distance
// replaces the name (re-parsed: "expected unqualified-id").
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 46:19 error "use of undeclared identifier 'valeu'"
//@ diag: 47:19 error "use of undeclared identifier 'valu'"
//@ diag: 48:19 error "use of undeclared identifier 'countr'; did you mean 'counter'?"
//@ diag: 40:5 note "'counter' declared here"
//@ diag: 49:19 error "use of undeclared identifier 'tmplf'"
//@ diag: 50:19 error "use of undeclared identifier 'retrun'; did you mean 'return'?"
//@ diag: 50:19 error "expected unqualified-id"
//@ diag: 51:19 error "use of undeclared identifier 'whlie'; did you mean 'while'?"
//@ diag: 51:19 error "expected unqualified-id"
//@ diag: 52:19 error "use of undeclared identifier 'tempalte'"
//@ diag: 53:19 error "use of undeclared identifier 'valuee'"
//@ diag: 54:19 error "use of undeclared identifier 'valq'"
//@ diag: 55:19 error "use of undeclared identifier 'vectr'"
//@ diag: 56:19 error "use of undeclared identifier 'Widgt'"
//@ diag: 57:19 error "use of undeclared identifier 'nsfnc'"
//@ diag: 58:29 error "use of undeclared identifier 'locva'; did you mean 'locvar'?"
//@ diag: 58:12 note "'locvar' declared here"
//@ diag: 59:20 error "use of undeclared identifier 'valu'"
//@ diag: 60:19 error "use of undeclared identifier 'sizeo'; did you mean 'sizeof'?"
//@ diag: 60:19 error "expected unqualified-id"
//@ diag: 61:19 error "use of undeclared identifier 'nullpt'; did you mean 'nullptr'?"
//@ diag: 61:19 error "expected unqualified-id"
//@ diag: 62:13 error "use of undeclared identifier 'whilee'; did you mean 'while'?"
//@ diag: 62:13 error "expected unqualified-id"
//@ diag: 63:13 error "use of undeclared identifier 'swtch'; did you mean 'switch'?"
//@ diag: 63:13 error "expected unqualified-id"
int value(int);
int vale(int); int valf(int);
int counter;
template<class T> int tmplfn(T);
template<class T> struct vector {};
struct Widget {};
namespace N { int nsfunc(int); }
using namespace N;
int a1() { return valeu<int> + 1; }
int a2() { return valu<int> + 1; }
int a3() { return countr<int> + 1; }
int a4() { return tmplf<int> + 1; }
int a5() { return retrun<int> + 1; }
int a6() { return whlie<int> + 1; }
int a7() { return tempalte<int> + 1; }
int a8() { return valuee<int>(1); }
int a9() { return valq<int> + 1; }
int b1() { return vectr<int> + 1; }
int b2() { return Widgt<int> + 1; }
int b3() { return nsfnc<int> + 1; }
int b4(int locvar) { return locva<int> + 1; }
int b5() { return &valu<int> == 0; }
int b6() { return sizeo<int> + 1; }
int b7() { return nullpt<int> + 1; }
void b8() { whilee<int> + 1; }
void b9() { swtch<int> + 1; }
