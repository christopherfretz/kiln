// [dcl.fct]/6 (CWG1417): a function type with a cv-qualifier-seq or a
// ref-qualifier, named through a typedef-name, is only the type of a
// non-static member function, of a typedef or alias-declaration, of a
// template type argument or of a type parameter's default, and the pointee
// of a pointer to member. As a non-member function at namespace or block
// scope, a static member function, a friend or a parameter it is an error
// at the start of the declaration and the qualifiers are dropped. Matches
// clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:1 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 24:1 error "non-member function of type 'G' (aka 'int () &') cannot have '&' qualifier"
//@ diag: 25:14 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 25:23 error "non-member function of type 'V' (aka 'int () const volatile &&') cannot have 'const volatile &&' qualifier"
//@ diag: 27:3 error "static member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 28:3 error "static member function of type 'F' (aka 'int () const') cannot have 'const' qualifier" dup(clang)
//@ diag: 29:3 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 31:14 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 33:13 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
typedef int F() const;
typedef int G() &;
typedef int V() const volatile &&;
F g;
G h;
void blk() { F local; V lv; }
struct S {
  static F sf;
  static F c, d;
  friend F fr;
  F ok, ok2;
  void param(F p);
};
void param2(F p);
