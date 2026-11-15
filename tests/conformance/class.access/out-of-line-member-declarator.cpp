// [class.access.general]/7, [basic.lookup.unqual]/3: in an out-of-line
// member definition only the names after the qualified declarator-id are in
// the member's scope and access context. A leading return type is not: an
// unqualified enclosing class there is undeclared. A non-member function
// naming the protected nested class is rejected, once (WP-1455).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:1 error "use of undeclared identifier 'I'"
//@ diag: 22:28 error "no viable conversion from returned value of type 'R' to function return type 'int'"
//@ diag: 23:11 error "'I' is a protected member of 'O'"
//@ diag: 14:10 note "declared protected here"
class O {
protected:
  struct I {
    struct R {
      R(I::R &&);
      I::R f();
    };
  };
};
O::I::R::R(I::R &&) {}
I::R O::I::R::f() { return static_cast<R &&>(*this); }
void g(O::I::R &);
