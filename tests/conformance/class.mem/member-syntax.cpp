// [class.mem.general]/1, [class.access.spec]/1, [class.virtual]/2, [class.bit]/1: member-declaration syntax errors
//@ expect: compile-fail
//@ diag: 16:10 error "expected ':'"
//@ diag: 17:8 error "expected ';' at end of declaration list"
//@ diag: 18:35 error "class member already marked 'override'"
//@ diag: 19:8 error "initializer on function does not look like a pure-specifier"
//@ diag: 20:12 error "expected ';' at end of declaration list"
//@ diag: 21:19 error "'final' is invalid in friend declarations"
//@ diag: 22:18 warning "duplicate 'const' declaration specifier"
//@ diag: 23:13 error "expected ';' at end of declaration list"
//@ diag: 24:11 error "anonymous bit-field cannot have a default member initializer"
//@ diag: 25:12 error "expected expression"
//@ diag: 26:19 error "cannot define a type in a friend declaration"
//@ diag: 28:16 error "class already marked 'final'"
struct S {
  private int a;
  int b public: int c;
  virtual void g() override final override;
  void h() = 1;
  int o : 3 final;
  friend void p() final;
  void q() const const;
  void r() & &&;
  int : 3 = 1;
  int bf : ;
  friend struct T { int x; };
};
struct F final final {};
