// [temp.names]/3: clang 19's delayed typo correction of an undeclared
// template-id (spec/sema.md "Undeclared template-ids") in classes: a member
// function is never a valid operand; 'break'/'continue'/'default' replace
// the name in a loop or switch, 'this' in a non-static member function.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 33:11 error "use of undeclared identifier 'tempalte'; did you mean 'template'?"
//@ diag: 33:11 error "'template' keyword not permitted here"
//@ diag: 33:25 error "expected unqualified-id"
//@ diag: 33:27 error "use of undeclared identifier 'virtuall'; did you mean 'virtual'?"
//@ diag: 33:27 error "expected unqualified-id"
//@ diag: 30:20 error "use of undeclared identifier 'membr'"
//@ diag: 31:20 error "use of undeclared identifier 'mem00'"
//@ diag: 32:27 error "use of undeclared identifier 'membr'"
//@ diag: 37:19 error "use of undeclared identifier 'tmplf'"
//@ diag: 38:19 error "use of undeclared identifier 'ovzz'"
//@ diag: 39:24 error "use of undeclared identifier 'breakk'; did you mean 'break'?"
//@ diag: 39:24 error "expected unqualified-id"
//@ diag: 39:41 error "use of undeclared identifier 'continu'; did you mean 'continue'?"
//@ diag: 39:41 error "expected unqualified-id"
//@ diag: 39:74 error "use of undeclared identifier 'defaul'; did you mean 'default'?"
//@ diag: 39:74 error "expected unqualified-id"
//@ diag: 39:69 warning "no case matching constant switch condition '0'"
//@ diag: 40:51 error "use of undeclared identifier 'thiss'; did you mean 'this'?"
//@ diag: 40:51 error "expected unqualified-id"
//@ diag: 41:22 error "use of undeclared identifier 'thiss'"
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
