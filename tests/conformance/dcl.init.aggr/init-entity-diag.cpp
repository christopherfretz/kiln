// [dcl.init.general]/16, [dcl.init.aggr]/4.2, [class.base.init]/7: a
// failed conversion names the initialized entity as clang's
// InitializedEntity kind: a member subobject (aggregate member, paren
// aggregate member, union member, mem-initializer, default member
// initializer, member of a base) and an array element. A variable inside
// a lambda in a default member initializer still names "a variable".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:6 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 31:6 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 32:15 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 33:7 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 34:13 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 35:15 error "cannot initialize an array element of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 36:12 error "cannot initialize an array element of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 37:39 error "cannot initialize an array element of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 40:7 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 40:13 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 41:26 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 42:21 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 43:31 error "cannot initialize a variable of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 44:20 error "cannot initialize a variable of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 47:36 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 48:40 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 49:19 error "cannot initialize return object of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 50:15 error "cannot initialize a new value of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 45:41 error "cannot initialize a member subobject of type 'int *' with an lvalue of type 'const char[2]'"
//@ diag: 46:9 note "in instantiation of member function 'G<int *>::G' requested here"
struct S { int *p; };
S s1{"x"};
S s2("x");
S *s3 = new S{"x"};
S s4{{"x"}};
S s5[1] = {{"y"}};
int *a1[1] = {"x"};
int *a2[2]("x", nullptr);
struct S3 { int *p[2]; } s7{{nullptr, "z"}};
struct A { int *p; };
struct D : A { int *q; };
D d1{{"x"}, "y"};
struct M { int *m; M() : m("x") {} };
struct E { int *m = "x"; };
struct F { int *m = [] { int *v = "x"; return v; }(); };
auto l = [] { int *v = "x"; return v; };
template<class T> struct G { T m; G() : m("x") {} };
G<int*> g;
struct U { union { int *u; }; } u1{"u"};
struct BF { int b : 3; int *p; } bf{1, "b"};
int *r() { return "x"; }
int **n = new int*("x");
