// [class.friend]/11: an unqualified friend operator function of a local class
// redeclares a block-scope operator declaration of the innermost enclosing
// block. Without one it is "no matching function found in local scope", with
// clang's near-match notes for the operators of an enclosing block; a namespace
// operator is not considered and typo correction suggests nothing.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:17 error "no matching function found in local scope"
//@ diag: 20:19 note "type of 1st parameter of local declaration does not match definition ('A *' vs 'A &')"
//@ diag: 25:17 error "no matching function found in local scope"
//@ diag: 26:17 error "no matching function found in local scope"
//@ diag: 33:19 error "no matching function found in local scope"
//@ diag: 21:8 note "local declaration nearly matches"
//@ diag: 37:16 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 21:8 note "previous declaration is here"
struct A {};
bool operator<=(A, A);
void f() {
  bool operator==(A, A);
  bool operator!=(A*, A);
  bool operator<(A, A);
  struct L {
    friend bool operator==(A, A);
    friend bool operator!=(A&, A);
    friend bool operator<=(A, A);
    friend bool operator>(A, A);
    struct N {
      friend bool operator==(A, A);
    };
  };
  {
    struct M {
      friend bool operator<(A, A);
    };
  }
  struct P {
    friend int operator<(A, A);
  };
}
