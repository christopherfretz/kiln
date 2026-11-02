// [class.virtual], [class.mem.general], [dcl.decl.general]: override
// control next to a requires-clause and outside a class (clang
// CheckOverrideControl, ParseCXXMemberDeclaratorBeforeInitializer). A
// virt-specifier and a requires-clause are alternatives of a
// member-declarator, so 'override requires' is a parse error after the
// specifier is checked; a virtual function's requires-clause is rejected
// again at instantiation. Outside a class 'override'/'final' are rejected
// before any virtual check, also on a non-virtual or static member and in
// a friend declaration.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 37:20 error "expected ';' at end of declaration list"
//@ diag: 38:29 error "virtual function cannot have a requires clause"
//@ diag: 39:12 error "only virtual member functions can be marked 'override'"
//@ diag: 39:20 error "expected ';' at end of declaration list"
//@ diag: 40:29 error "virtual function cannot have a requires clause"
//@ diag: 41:12 error "only virtual member functions can be marked 'final'"
//@ diag: 41:17 error "expected ';' at end of declaration list"
//@ diag: 38:29 error "virtual function cannot have a requires clause"
//@ diag: 43:8 note "in instantiation of template class 'D<int>' requested here"
//@ diag: 40:29 error "virtual function cannot have a requires clause"
//@ diag: 46:20 error "expected ';' at end of declaration list"
//@ diag: 47:12 error "only virtual member functions can be marked 'override'"
//@ diag: 47:20 error "expected ';' at end of declaration list"
//@ diag: 48:19 error "only virtual member functions can be marked 'override'"
//@ diag: 48:27 error "expected ';' at end of declaration list"
//@ diag: 52:13 error "'override' specifier is not allowed outside a class definition"
//@ diag: 53:13 error "'final' specifier is not allowed outside a class definition"
//@ diag: 54:13 error "'override' specifier is not allowed outside a class definition"
//@ diag: 55:13 error "'final' specifier is not allowed outside a class definition"
//@ diag: 56:16 error "'override' specifier is not allowed outside a class definition"
//@ diag: 57:35 error "'override' is invalid in friend declarations"
struct B { virtual void f(); virtual void g(); };

template <class T> struct D : B {
  void f() override requires true;
  virtual void g() requires true override;
  void h() override requires true;
  virtual void k() requires (sizeof(T) > 0);
  void m() final requires true;
};
D<int> d;

struct E : B {
  void f() override requires true;
  void h() override requires true;
  static void s() override requires true;
};

struct F : B { void f() override; void g() final; void h(); static void s(); };
void F::f() override {}
void F::g() final {}
void F::h() override {}
void F::s() final {}
void free_fn() override;
struct G : B { friend void B::f() override; };
