// [dcl.contract.func] (P2900R14, as GCC 16.1 reads it): a virtual function,
// whether declared virtual or overriding, cannot have a
// function-contract-specifier-seq. GCC 16.1 rejects these too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 11:24 error "function contract specifiers cannot be added to virtual functions"
//@ diag: 14:40 error "function contract specifiers cannot be added to virtual functions"
struct B {
  virtual int v(int a) pre(a > 0) = 0;
  virtual void w() const;
};
struct D : B { void w() const override pre(true); };
