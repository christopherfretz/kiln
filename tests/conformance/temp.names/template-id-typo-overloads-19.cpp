// [temp.names]/3: clang 19's delayed typo correction of an undeclared
// template-id (spec/sema.md "Undeclared template-ids"): a function or
// overload set, member or qualified, is an invalid operand; a value at the
// least distance (a local variable, an enumerator) is corrected; a variable
// template is no candidate. (A keyword and a function at one distance are
// tried in clang's hash order, which kiln does not model.)
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 35:71 error "use of undeclared identifier 'membr'"
//@ diag: 35:106 error "use of undeclared identifier 'smembr'"
//@ diag: 36:33 error "use of undeclared identifier 'valc'; did you mean 'vala'?"
//@ diag: 36:16 note "'vala' declared here"
//@ diag: 37:19 error "use of undeclared identifier 'mfunc'"
//@ diag: 38:19 error "use of undeclared identifier 'mfuncc'"
//@ diag: 39:19 error "use of undeclared identifier 'vtmpll'"
//@ diag: 40:19 error "use of undeclared identifier 'enumr'; did you mean 'enumer'?"
//@ diag: 34:10 note "'enumer' declared here"
//@ diag: 41:19 error "use of undeclared identifier 'mfxy'"
//@ diag: 42:63 error "use of undeclared identifier 'pfnc'"
//@ diag: 43:33 error "use of undeclared identifier 'valbb'; did you mean 'valb'?"
//@ diag: 43:16 note "'valb' declared here"
//@ diag: 50:18 error "use of undeclared identifier 'faa'"
//@ diag: 50:29 error "use of undeclared identifier 'fbb'"
//@ diag: 50:40 error "use of undeclared identifier 'fcc'"
//@ diag: 50:51 error "use of undeclared identifier 'fdd'"
//@ diag: 50:62 error "use of undeclared identifier 'fee'"
//@ diag: 50:73 error "use of undeclared identifier 'fff'"
int valb(int);
namespace M { int mfunc(int); int mfx(int); }
int swatch(int);
template<class T> int vtmpl = 0;
enum E { enumer };
struct S { int member(int); static int smember(int); int g() { return membr<int> + 1; } int h() { return smembr<int> + 1; } };
int d1() { int vala = 0; return valc<int> + 1; }
int d2() { return mfunc<int> + 1; }
int d3() { return mfuncc<int> + 1; }
int d5() { return vtmpll<int> + 1; }
int d6() { return enumr<int> + 1; }
int d7() { return mfxy<int> + 1; }
namespace P { int pfunc(int); namespace Q { int qq() { return pfnc<int> + 1; } } }
int d8() { int valb = 0; return valbb<int> + 1; }
int fa(int); int fa(char = 0); int fa();
int fb(); int fb(char = 0);
int fc(char = 0); int fc();
int fd(int); int fd(char); int fd(long);
int fe(int); int fe(char); int fe(long); int fe(short);
int ff(int, ...); int ff(...); int ff(char);
int a() { return faa<int> + fbb<int> + fcc<int> + fdd<int> + fee<int> + fff<int>; }
