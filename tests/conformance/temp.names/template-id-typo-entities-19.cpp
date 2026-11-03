// [temp.names]/3: clang 19's delayed typo correction of an undeclared
// template-id (spec/sema.md "Undeclared template-ids") by kind of entity:
// a field with an implicit object, enumerator, parameter or non-type
// template parameter (even with dependent arguments) is corrected; a
// function, typedef, namespace, concept or alias template is not; 'template'
// at namespace scope or in a default member initializer is "not permitted
// here" and the operand after it is an "expected unqualified-id".
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 39:40 error "use of undeclared identifier 'feld'; did you mean 'field'?"
//@ diag: 39:16 note "'field' declared here"
//@ diag: 45:19 error "use of undeclared identifier 'val'"
//@ diag: 46:19 error "use of undeclared identifier 'colur'; did you mean 'colour'?"
//@ diag: 38:10 note "'colour' declared here"
//@ diag: 47:19 error "use of undeclared identifier 'mynt'"
//@ diag: 48:19 error "use of undeclared identifier 'alpa'"
//@ diag: 49:19 error "use of undeclared identifier 'ovzz'"
//@ diag: 50:19 error "use of undeclared identifier 'Cptt'"
//@ diag: 51:10 error "use of undeclared identifier 'tempalte'; did you mean 'template'?"
//@ diag: 51:10 error "'template' keyword not permitted here"
//@ diag: 51:24 error "expected unqualified-id"
//@ diag: 52:20 error "use of undeclared identifier 'virtuall'; did you mean 'virtual'?"
//@ diag: 52:20 error "expected unqualified-id"
//@ diag: 52:47 error "use of undeclared identifier 'tempalte'; did you mean 'template'?"
//@ diag: 52:47 error "'template' keyword not permitted here"
//@ diag: 52:61 error "expected unqualified-id"
//@ diag: 53:20 error "use of undeclared identifier 'Alis'"
//@ diag: 54:21 error "use of undeclared identifier 'feld'"
//@ diag: 55:29 error "use of undeclared identifier 'parm'; did you mean 'param'?"
//@ diag: 55:13 note "'param' declared here"
//@ diag: 56:45 error "use of undeclared identifier 'parm'; did you mean 'param'?"
//@ diag: 56:29 note "'param' declared here"
//@ diag: 57:39 error "use of undeclared identifier 'NTP'; did you mean 'NTTP'?"
//@ diag: 57:14 note "'NTTP' declared here"
int val(int); int vat(int);
enum E { colour };
struct S { int field; int g() { return feld<int> + 1; } };
typedef int myint;
namespace alpha {}
int ovz(int); int ovz();
template<class T> concept Cpt = true;
template<class T> using Alias = T;
int a1() { return val<int> + 1; }
int a2() { return colur<int> + 1; }
int a4() { return mynt<int> + 1; }
int a5() { return alpa<int> + 1; }
int a6() { return ovzz<int> + 1; }
int a8() { return Cptt<int> + 1; }
int x9 = tempalte<int> + 1;
struct T { int m = virtuall<int> + 1; int n = tempalte<int> + 1; };
int a11() { return Alis<int> + 1; }
int a12() { return &feld<int> == 0; }
int a13(int param) { return parm<int> + 1; }
template<class U> int a14(U param) { return parm<U> + 1; }
template<int NTTP> int a15() { return NTP<int> + 1; }
