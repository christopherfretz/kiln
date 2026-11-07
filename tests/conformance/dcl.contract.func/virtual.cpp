// [dcl.contract.func]/6 (the Croydon DIS, P2900R14): "A virtual function
// ... shall not have a function-contract-specifier-seq", whether declared
// virtual or overriding. GCC 16.1 rejects these too. (P3097 lifts the
// rule for C++29; spec/contracts.md §9.)
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 12:24 error "function contract specifiers cannot be added to virtual functions"
//@ diag: 15:40 error "function contract specifiers cannot be added to virtual functions"
struct B {
  virtual int v(int a) pre(a > 0) = 0;
  virtual void w() const;
};
struct D : B { void w() const override pre(true); };
