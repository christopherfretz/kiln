// [class.copy.ctor]/10: a defaulted copy constructor is deleted for an rvalue
// reference member and a subobject whose copy constructor is deleted,
// inaccessible or not found (a member that only has a move constructor);
// [dcl.fct.def.default]/2: a const or volatile parameter, extra defaulted parameters.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:3 warning "explicitly defaulted copy constructor is implicitly deleted"
//@ diag: 28:5 note "copy constructor of 'MC' is implicitly deleted because field 'm' has a deleted copy constructor"
//@ diag: 26:12 note "copy constructor is implicitly deleted because 'M' has a user-declared move constructor"
//@ diag: 29:20 note "replace 'default' with 'delete'"
//@ diag: 33:3 warning "explicitly defaulted copy constructor is implicitly deleted"
//@ diag: 32:9 note "copy constructor of 'RR' is implicitly deleted because field 'r' is of rvalue reference type 'int &&'"
//@ diag: 33:20 note "replace 'default' with 'delete'"
//@ diag: 37:3 warning "explicitly defaulted copy constructor is implicitly deleted"
//@ diag: 36:13 note "copy constructor of 'DB' is implicitly deleted because base class 'NCB' has a deleted copy constructor"
//@ diag: 35:14 note "'NCB' has been explicitly marked deleted here"
//@ diag: 37:20 note "replace 'default' with 'delete'"
//@ diag: 42:3 warning "explicitly defaulted copy constructor is implicitly deleted"
//@ diag: 41:6 note "copy constructor of 'HP' is implicitly deleted because field 'p' has an inaccessible copy constructor"
//@ diag: 42:20 note "replace 'default' with 'delete'"
//@ diag: 47:3 warning "explicitly defaulted copy constructor is implicitly deleted"
//@ diag: 47:3 note "function is implicitly deleted because its declared type does not match the type of an implicit copy constructor"
//@ diag: 49:15 warning "explicitly defaulted copy constructor is implicitly deleted"
//@ diag: 49:15 note "function is implicitly deleted because its declared type does not match the type of an implicit copy constructor"
//@ diag: 51:15 error "an explicitly-defaulted copy constructor cannot have default arguments"
struct M { M(M &&); };
struct MC {
  M m;
  MC(const MC &) = default;
};
struct RR {
  int &&r;
  RR(const RR &) = default;
};
struct NCB { NCB(const NCB &) = delete; };
struct DB : NCB {
  DB(const DB &) = default;
};
struct PC { private: PC(const PC &); };
struct HP {
  PC p;
  HP(const HP &) = default;
};
struct NonConst { NonConst(NonConst &); };
struct NCHolder {
  NonConst n;
  NCHolder(const NCHolder &) = default;
};
struct VolP { VolP(const volatile VolP &) = default; };
struct Ok { Ok(const Ok &) = default; Ok(Ok &&) = default; int i; };
struct Args { Args(const Args &, int = 0) = default; };
