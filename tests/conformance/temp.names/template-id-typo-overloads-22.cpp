// [temp.names]/3: clang 22's typo correction of an undeclared template-id
// (spec/sema.md "Undeclared template-ids"): the qualifier is spelled from
// the current context (global when the path starts with an enclosing
// name); an overload set is named by its one function callable with no
// arguments, else the last declared; a non-static member function "must be
// called" (or has no object argument from a static one); a variable or
// enumerator outside a template is "expected to be a non-type template";
// a variable template is no correction.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 55:71 error "use of undeclared identifier 'membr'; did you mean 'member'?"
//@ diag: 55:16 note "'member' declared here"
//@ diag: 55:71 error "reference to non-static member function must be called"
//@ diag: 55:106 error "use of undeclared identifier 'smembr'; did you mean 'smember'?"
//@ diag: 55:40 note "'smember' declared here"
//@ diag: 55:118 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 56:33 error "use of undeclared identifier 'valc'"
//@ diag: 57:19 error "use of undeclared identifier 'mfunc'; did you mean 'M::mfunc'?"
//@ diag: 51:19 note "'M::mfunc' declared here"
//@ diag: 57:30 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 58:19 error "use of undeclared identifier 'mfuncc'; did you mean 'M::mfunc'?"
//@ diag: 51:19 note "'M::mfunc' declared here"
//@ diag: 58:31 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 59:13 error "use of undeclared identifier 'swtch'"
//@ diag: 60:19 error "use of undeclared identifier 'vtmpll'"
//@ diag: 61:19 error "use of undeclared identifier 'enumr'"
//@ diag: 62:19 error "use of undeclared identifier 'mfxy'"
//@ diag: 63:63 error "use of undeclared identifier 'pfnc'; did you mean 'pfunc'?"
//@ diag: 63:19 note "'pfunc' declared here"
//@ diag: 63:73 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 64:33 error "use of undeclared identifier 'valbb'; did you mean 'valb'?"
//@ diag: 64:16 note "'valb' declared here"
//@ diag: 64:33 error "'valb' is expected to be a non-type template, but instantiated to a class template"
//@ diag: 64:16 note "class template declared here"
//@ diag: 71:18 error "use of undeclared identifier 'faa'; did you mean 'fa'?"
//@ diag: 65:36 note "'fa' declared here"
//@ diag: 71:29 error "use of undeclared identifier 'fbb'; did you mean 'fb'?"
//@ diag: 66:15 note "'fb' declared here"
//@ diag: 71:27 error "invalid operands to binary expression ('int ()' and 'int (char)')"
//@ diag: 71:40 error "use of undeclared identifier 'fcc'; did you mean 'fc'?"
//@ diag: 67:23 note "'fc' declared here"
//@ diag: 71:51 error "use of undeclared identifier 'fdd'; did you mean 'fd'?"
//@ diag: 68:32 note "'fd' declared here"
//@ diag: 71:62 error "use of undeclared identifier 'fee'; did you mean 'fe'?"
//@ diag: 69:46 note "'fe' declared here"
//@ diag: 71:73 error "use of undeclared identifier 'fff'; did you mean 'ff'?"
//@ diag: 70:23 note "'ff' declared here"
int valb(int);
namespace M { int mfunc(int); int mfx(int); }
int swatch(int);
template<class T> int vtmpl = 0;
enum E { enumer };
struct S { int member(int); static int smember(int); int g() { return membr<int> + 1; } int h() { return smembr<int> + 1; } };
int d1() { int vala = 0; return valc<int> + 1; }
int d2() { return mfunc<int> + 1; }
int d3() { return mfuncc<int> + 1; }
void d4() { swtch<int> + 1; }
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
