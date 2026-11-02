// [temp.names]/3, [class.member.lookup]: an undeclared template-id naming
// a member function of an enclosing class (ordinary lookup sets the
// non-template aside) is "member 'f' used before its declaration" when
// the class declares it, "explicit qualification required to use member
// 'f' from dependent base class" when it inherits it, or when the one
// function callable with no arguments was picked out (clang 22's
// DiagnoseEmptyLookup); then the recovery of a correction to it. Members
// of unions and class templates are no typo correction.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 67:19 error "use of undeclared identifier 'rfunc'; did you mean 'R::rfunc'?"
//@ diag: 61:23 note "'R::rfunc' declared here"
//@ diag: 67:30 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 68:19 error "use of undeclared identifier 'rfnc'"
//@ diag: 69:19 error "use of undeclared identifier 'rmem'; did you mean 'R::rmem'?"
//@ diag: 61:39 note "'R::rmem' declared here"
//@ diag: 69:19 error "call to non-static member function without an object argument"
//@ diag: 70:19 error "use of undeclared identifier 'tmplf'; did you mean 'tmplfn'?"
//@ diag: 62:23 note "'tmplfn' declared here"
//@ diag: 70:30 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 71:19 error "use of undeclared identifier 'rvar'; did you mean 'R::rvar'?"
//@ diag: 61:61 note "'R::rvar' declared here"
//@ diag: 71:19 error "'rvar' is expected to be a non-type template, but instantiated to a class template"
//@ diag: 61:61 note "class template declared here"
//@ diag: 72:19 error "use of undeclared identifier 'ufn'"
//@ diag: 73:19 error "use of undeclared identifier 'ttfn'"
//@ diag: 74:21 error "member 'sfn' used before its declaration"
//@ diag: 66:23 note "member is declared here"
//@ diag: 74:30 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 75:62 error "member 'sfn2' used before its declaration"
//@ diag: 75:24 note "member is declared here"
//@ diag: 75:74 error "use of undeclared identifier 'fld'"
//@ diag: 75:107 error "'fld2' does not name a template but is followed by template arguments"
//@ diag: 75:39 note "non-template declaration found by name lookup"
//@ diag: 75:147 error "'fld3' does not name a template but is followed by template arguments"
//@ diag: 75:174 note "non-template declaration found by name lookup"
//@ diag: 76:51 error "use of undeclared identifier 'locw'; did you mean 'locv'?"
//@ diag: 76:34 note "'locv' declared here"
//@ diag: 77:51 error "use of undeclared identifier 'locw'; did you mean 'locv'?"
//@ diag: 77:34 note "'locv' declared here"
//@ diag: 81:20 error "member 'ds' used before its declaration"
//@ diag: 80:14 note "member is declared here"
//@ diag: 81:30 error "member 'dm' used before its declaration"
//@ diag: 80:27 note "member is declared here"
//@ diag: 81:30 error "reference to non-static member function must be called"
//@ diag: 81:40 error "explicit qualification required to use member 'bs' from dependent base class"
//@ diag: 78:23 note "member is declared here"
//@ diag: 81:50 error "explicit qualification required to use member 'bm' from dependent base class"
//@ diag: 78:36 note "member is declared here"
//@ diag: 81:50 error "reference to non-static member function must be called"
//@ diag: 82:27 error "member 'ds' used before its declaration"
//@ diag: 80:14 note "member is declared here"
//@ diag: 82:37 error "member 'dm' used before its declaration"
//@ diag: 80:27 note "member is declared here"
//@ diag: 82:37 error "call to non-static member function without an object argument"
//@ diag: 83:20 error "explicit qualification required to use member 'dm0' from dependent base class"
//@ diag: 80:40 note "member is declared here"
//@ diag: 83:20 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
struct R { static int rfunc(int); int rmem(int); static int rvar; };
template<class T> int tmplfn(T);
int glob;
union UU { static int ufn(int); };
template<class T> struct TT { static int ttfn(int); };
struct S { static int sfn(int); int fld; int g(); };
int a1() { return rfunc<int> + 1; }
int a2() { return rfnc<int> + 1; }
int a3() { return rmem<int> + 1; }
int a4() { return tmplf<int> + 1; }
int a5() { return rvar<int> + 1; }
int a6() { return ufn<int> + 1; }
int a7() { return ttfn<int> + 1; }
int S::g() { return sfn<int> + 1; }
struct S2 { static int sfn2(int); int fld2; int g() { return sfn2<int> + fld<int> + 1; } int h() { return fld2<int> * nullptr; } int k() { return fld3<int> * nullptr; } int fld3; };
template<class U> int t1() { int locv = 0; return locw<int> + 1; }
template<class U> int t2() { int locv = 0; return locw<U> + 1; }
struct B { static int bs(int); int bm(int); };
struct D : B {
  static int ds(int); int dm(int); int dm0();
  int g() { return ds<int> + dm<int> + bs<int> + bm<int>; }
  static int h() { return ds<int> + dm<int>; }
  int k() { return dm0<int> + 1; }
};
