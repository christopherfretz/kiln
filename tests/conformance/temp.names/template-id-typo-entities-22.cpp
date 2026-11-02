// [temp.names]/3: clang 22's typo correction of an undeclared template-id
// (spec/sema.md "Undeclared template-ids") by kind of entity: a typedef,
// alias template (noted at its 'using') or keyword is only suggested, a
// field or template parameter suggested with no follow-on, a namespace
// or concept is no correction; the keywords offered depend on the scope
// (member specifiers in a default member initializer).
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 43:40 error "use of undeclared identifier 'feld'; did you mean 'field'?"
//@ diag: 43:16 note "'field' declared here"
//@ diag: 49:19 error "use of undeclared identifier 'val'; did you mean 'vat'?"
//@ diag: 41:19 note "'vat' declared here"
//@ diag: 49:28 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 50:19 error "use of undeclared identifier 'colur'; did you mean 'colour'?"
//@ diag: 42:10 note "'colour' declared here"
//@ diag: 50:19 error "'colour' is expected to be a non-type template, but instantiated to a class template"
//@ diag: 42:10 note "class template declared here"
//@ diag: 51:19 error "use of undeclared identifier 'mynt'; did you mean 'myint'?"
//@ diag: 44:13 note "'myint' declared here"
//@ diag: 52:19 error "use of undeclared identifier 'alpa'"
//@ diag: 53:19 error "use of undeclared identifier 'ovzz'; did you mean 'ovz'?"
//@ diag: 46:19 note "'ovz' declared here"
//@ diag: 53:29 error "arithmetic on a pointer to the function type 'int ()'"
//@ diag: 54:19 error "use of undeclared identifier 'Cptt'"
//@ diag: 55:10 error "use of undeclared identifier 'tempalte'; did you mean 'template'?"
//@ diag: 56:20 error "use of undeclared identifier 'virtuall'; did you mean 'virtual'?"
//@ diag: 56:47 error "use of undeclared identifier 'tempalte'; did you mean 'template'?"
//@ diag: 57:20 error "use of undeclared identifier 'Alis'; did you mean 'Alias'?"
//@ diag: 48:19 note "'Alias' declared here"
//@ diag: 58:21 error "use of undeclared identifier 'feld'"
//@ diag: 59:29 error "use of undeclared identifier 'parm'; did you mean 'param'?"
//@ diag: 59:13 note "'param' declared here"
//@ diag: 59:29 error "'param' is expected to be a non-type template, but instantiated to a class template"
//@ diag: 59:13 note "class template declared here"
//@ diag: 60:45 error "use of undeclared identifier 'parm'; did you mean 'param'?"
//@ diag: 60:29 note "'param' declared here"
//@ diag: 61:39 error "use of undeclared identifier 'NTP'; did you mean 'NTTP'?"
//@ diag: 61:14 note "'NTTP' declared here"
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
