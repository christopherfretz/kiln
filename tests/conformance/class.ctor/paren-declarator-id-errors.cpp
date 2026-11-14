// [class.ctor.general]/1, [dcl.ambig.res]/1, [class.mem.general]: what
// follows the class name and `(` decides between a constructor and a
// parenthesized declarator, as clang's isConstructorDeclarator does.
// `U (a);` with `a` not a type is a constructor with an unknown parameter
// type; `V((int));` is not a parameter list, so `V` is the decl-specifier
// and `(int)` no declarator; `static S (::g)(int);` declares the
// non-friend member `g` with a qualified name; at namespace scope
// `Y (::gy)(int);` redeclares `::gy`, which does not exist.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:6 error "unknown type name 'a'"
//@ diag: 25:6 error "expected member name or ';' after declaration specifiers"
//@ diag: 25:6 error "expected ')'"
//@ diag: 25:5 note "to match this '('"
//@ diag: 30:15 error "non-friend class member 'g' cannot have a qualified name"
//@ diag: 33:6 warning "extra qualification on member 'gy'"
//@ diag: 33:6 error "out-of-line declaration of 'gy' does not match any declaration in the global namespace"
struct S;
S g(int);
struct U {
  int k;
  U (a);
};
struct V {
  V((int));
  V(int (x));
};
struct W;
struct S {
  static S (::g)(int);
};
struct Y { Y(int); };
Y (::gy)(int);
