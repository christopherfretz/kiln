// [temp.names]/3: clang 22's typo correction of an undeclared template-id
// (spec/sema.md "Undeclared template-ids"): 'this' is offered only in a
// non-static member function, 'break'/'continue'/'default' only in a loop
// or switch; a function template is recovered with its template arguments
// (the specialization a built-in operator then sees).
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 38:11 error "use of undeclared identifier 'tempalte'; did you mean 'template'?"
//@ diag: 38:27 error "use of undeclared identifier 'virtuall'; did you mean 'virtual'?"
//@ diag: 38:43 error "use of undeclared identifier 'thiss'"
//@ diag: 35:20 error "use of undeclared identifier 'membr'; did you mean 'member'?"
//@ diag: 34:16 note "'member' declared here"
//@ diag: 35:20 error "reference to non-static member function must be called"
//@ diag: 36:20 error "use of undeclared identifier 'mem00'; did you mean 'mem0'?"
//@ diag: 34:33 note "'mem0' declared here"
//@ diag: 36:20 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 37:27 error "use of undeclared identifier 'membr'; did you mean 'member'?"
//@ diag: 34:16 note "'member' declared here"
//@ diag: 37:27 error "call to non-static member function without an object argument"
//@ diag: 42:19 error "use of undeclared identifier 'tmplf'; did you mean 'tmplfn'?"
//@ diag: 40:38 note "'tmplfn' declared here"
//@ diag: 42:36 error "arithmetic on a pointer to the function type 'int (int, char)'"
//@ diag: 43:19 error "use of undeclared identifier 'ovzz'; did you mean 'ovz'?"
//@ diag: 41:30 note "'ovz' declared here"
//@ diag: 43:29 error "arithmetic on a pointer to the function type 'int (char)'"
//@ diag: 44:24 error "use of undeclared identifier 'breakk'; did you mean 'break'?"
//@ diag: 44:41 error "use of undeclared identifier 'continu'; did you mean 'continue'?"
//@ diag: 44:74 error "use of undeclared identifier 'defaul'; did you mean 'default'?"
//@ diag: 44:69 warning "no case matching constant switch condition '0'"
//@ diag: 45:51 error "use of undeclared identifier 'thiss'; did you mean 'this'?"
//@ diag: 46:22 error "use of undeclared identifier 'thiss'"
struct S { int member(int); int mem0(); static int smember(int);
  int g() { return membr<int> + 1; }
  int h() { return mem00<int> + 1; }
  static int k() { return membr<int> + 1; }
  int m = tempalte<int> + virtuall<int> + thiss<int>;
};
template<class T, class U = int> int tmplfn(T, U = 0);
int ovz(int); int ovz(); int ovz(char = 0);
int a1() { return tmplf<int, char> + 1; }
int a2() { return ovzz<int> + 1; }
void a3() { for (;;) { breakk<int> + 1; continu<int> + 1; } switch (0) { defaul<int> + 1; } }
template<class T> struct Tmpl { int tf() { return thiss<int> + 1; } };
int a4(int) { return thiss<int> + 1; }
