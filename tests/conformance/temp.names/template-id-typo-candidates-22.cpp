// [temp.names]/3: a template-id whose name ordinary lookup does not find
// (or finds only functions for), not called, names nothing. clang 22
// typo-corrects the name (spec/sema.md "Undeclared template-ids"):
// candidates are the visible names and the keywords within (len+2)/3
// character edits, plus the name qualified by a namespace or class that
// declares it. The least distance wins; a tie with another candidate
// (valu: value / vale / valf) means no correction, as does a winner more
// than a third of the name away (valeu: two edits in five characters).
// A function is recovered as its overload set; a type or keyword is only
// suggested.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 55:19 error "use of undeclared identifier 'valeu'; did you mean 'vale'?"
//@ diag: 48:5 note "'vale' declared here"
//@ diag: 55:30 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 56:19 error "use of undeclared identifier 'valu'"
//@ diag: 57:19 error "use of undeclared identifier 'countr'; did you mean 'counter'?"
//@ diag: 49:5 note "'counter' declared here"
//@ diag: 57:19 error "'counter' is expected to be a non-type template, but instantiated to a class template"
//@ diag: 49:5 note "class template declared here"
//@ diag: 58:19 error "use of undeclared identifier 'tmplf'; did you mean 'tmplfn'?"
//@ diag: 50:23 note "'tmplfn' declared here"
//@ diag: 58:30 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 59:19 error "use of undeclared identifier 'retrun'; did you mean 'return'?"
//@ diag: 60:19 error "use of undeclared identifier 'whlie'"
//@ diag: 61:19 error "use of undeclared identifier 'tempalte'"
//@ diag: 62:19 error "use of undeclared identifier 'valuee'"
//@ diag: 63:19 error "use of undeclared identifier 'valq'"
//@ diag: 64:19 error "use of undeclared identifier 'vectr'; did you mean 'vector'?"
//@ diag: 51:26 note "'vector' declared here"
//@ diag: 65:19 error "use of undeclared identifier 'Widgt'; did you mean 'Widget'?"
//@ diag: 52:8 note "'Widget' declared here"
//@ diag: 66:19 error "use of undeclared identifier 'nsfnc'; did you mean 'nsfunc'?"
//@ diag: 53:19 note "'nsfunc' declared here"
//@ diag: 66:30 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 67:29 error "use of undeclared identifier 'locva'; did you mean 'locvar'?"
//@ diag: 67:12 note "'locvar' declared here"
//@ diag: 67:29 error "'locvar' is expected to be a non-type template, but instantiated to a class template"
//@ diag: 67:12 note "class template declared here"
//@ diag: 68:20 error "use of undeclared identifier 'valu'"
//@ diag: 69:19 error "use of undeclared identifier 'sizeo'; did you mean 'sizeof'?"
//@ diag: 70:19 error "use of undeclared identifier 'nullpt'; did you mean 'nullptr'?"
//@ diag: 71:13 error "use of undeclared identifier 'whilee'; did you mean 'while'?"
//@ diag: 72:13 error "use of undeclared identifier 'swtch'; did you mean 'switch'?"
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
