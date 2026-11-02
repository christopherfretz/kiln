// [temp.res.general]/3, [dcl.type.pack.index]: a typename-specifier names
// a nested-name-specifier, so `typename T` and `typename Ts...[0]` are
// "expected a qualified name after 'typename'". The recovery follows
// clang's: in a block and in a type-id the name (and its pack index) is
// consumed as an erroneous type; a namespace-scope or member declaration
// ends at `typename Ts...[0]` ("expected unqualified-id" / "expected member
// name or ';'"); an alias-declaration's type-id is reported unterminated;
// a parameter list ends at the name ("expected ')'"; the parameter is not
// declared and the function is invalid), and the first parameter of a
// namespace- or block-scope declarator, which clang disambiguates
// tentatively, reports the name twice. clang 19 and clang 22 agree.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 53:12 error "expected a qualified name after 'typename'"
//@ diag: 55:19 error "expected a qualified name after 'typename'"
//@ diag: 56:25 error "expected a qualified name after 'typename'"
//@ diag: 57:33 error "expected a qualified name after 'typename'"
//@ diag: 62:33 error "expected a qualified name after 'typename'"
//@ diag: 62:33 error "expected unqualified-id"
//@ diag: 64:12 error "expected a qualified name after 'typename'"
//@ diag: 64:12 error "expected member name or ';' after declaration specifiers"
//@ diag: 66:19 error "expected a qualified name after 'typename'"
//@ diag: 66:19 error "expected ')'"
//@ diag: 66:9 note "to match this '('"
//@ diag: 71:43 error "expected a qualified name after 'typename'"
//@ diag: 71:51 error "expected ';' after alias declaration"
//@ diag: 75:41 error "expected a qualified name after 'typename'"
//@ diag: 75:41 error "expected a qualified name after 'typename'"
//@ diag: 75:41 error "expected ')'"
//@ diag: 75:31 note "to match this '('"
//@ diag: 76:46 error "expected a qualified name after 'typename'"
//@ diag: 76:46 error "expected ')'"
//@ diag: 76:31 note "to match this '('"
//@ diag: 77:37 error "expected a qualified name after 'typename'"
//@ diag: 77:37 error "expected a qualified name after 'typename'"
//@ diag: 77:37 error "expected ')'"
//@ diag: 77:27 note "to match this '('"
//@ diag: 78:42 error "expected a qualified name after 'typename'"
//@ diag: 80:19 error "expected a qualified name after 'typename'"
//@ diag: 83:19 error "expected a qualified name after 'typename'"
//@ diag: 83:19 error "expected a qualified name after 'typename'"
//@ diag: 83:19 error "expected ')'"
//@ diag: 83:9 note "to match this '('"
//@ diag: 84:24 error "expected a qualified name after 'typename'"
//@ diag: 87:24 error "expected a qualified name after 'typename'"
//@ diag: 87:24 error "expected ')'"
//@ diag: 87:14 note "to match this '('"
//@ diag: 89:14 error "no matching function for call to 'p1'"
//@ diag: 92:56 error "expected a qualified name after 'typename'"
// Block scope and type-ids: the name and its pack index are consumed.
template <class... Ts> int block() {
  typename Ts...[0] a, b;
  a = 1;
  (void)(typename Ts...[0])0;
  int *p = new typename Ts...[0];
  static_assert(sizeof(typename Ts...[0]) == sizeof(int));
  return a + b + *p;
}

// Namespace scope and class members: the declaration ends at the name.
template <class... Ts> typename Ts...[0] v, w;
template <class... Ts> struct S {
  typename Ts...[0] m, n;
  int q;
  void g(typename Ts...[0] a);
  void h() { q = 2; }
};

// Alias declarations: the type-id is unterminated.
template <class... Ts> using A = typename Ts...[0];

// Parameters: the parameter list ends at the name; the first parameter
// of a namespace- or block-scope declarator is reported twice.
template <class... Ts> void p1(typename Ts...[0] a, int b);
template <class... Ts> void p2(int, typename Ts...[0], int);
template <class T> void p3(typename T a);
template <class T> void p4(int, typename T a);
template <class T> struct P {
  void f(typename T a);
};
template <class T> void p5() {
  void g(typename T a);
  auto l = [](typename T a) { return a; };
}
template <class... Ts> void p6() {
  auto l = [](typename Ts...[0] a) {};
}
void use() { p1<int>(1, 2); }

// A type-requirement names a type-name, not a pack-index-specifier.
template <class... Ts> concept C = requires { typename Ts...[0]; };

int after = 0;
