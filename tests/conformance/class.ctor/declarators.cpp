// [class.ctor.general]/1-2, [class.mem.general]/23: a constructor has no return type,
// cv- or ref-qualifier, and is not static or virtual; [class.copy.ctor]/6: a
// copy constructor taking its class by value; [class.mem.general]/5 redeclaration.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:7 error "constructor cannot have a return type"
//@ diag: 16:7 error "'const' qualifier is not allowed on a constructor"
//@ diag: 17:7 error "ref-qualifier '&' is not allowed on a constructor"
//@ diag: 18:10 error "constructor cannot be declared 'static'"
//@ diag: 19:11 error "constructor cannot be declared 'virtual'"
//@ diag: 20:6 error "copy constructor must pass its first argument by reference"
//@ diag: 25:3 error "constructor cannot be redeclared"
//@ diag: 24:3 note "previous declaration is here"
struct S {
  int S();
  S() const;
  S() &;
  static S(int);
  virtual S(char);
  S(S);
};
struct T {
  T(T &, int = 0);
  T();
  T();
};
