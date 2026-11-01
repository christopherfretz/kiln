// [dcl.init.list]/3.7, [over.match.list]: `E e = {1};` as a default
// member initializer is copy-list-initialization and rejects an explicit
// constructor (`E e{1};` does not).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:9 error "chosen constructor is explicit in copy-initialization"
//@ diag: 11:21 note "explicit constructor declared here"
//@ diag: 15:12 error "cannot initialize a member subobject of type 'int' with an lvalue of type 'const char[2]'"
//@ diag: 18:18 error "chosen constructor is explicit in copy-initialization"
//@ diag: 11:21 note "explicit constructor declared here"
struct E { explicit E(int); };
struct S {
  E e = {1};
  E f{1};
  int g = {"x"};
};
S s;
struct T { E e = {1}; T() {} };
