// [dcl.attr.grammar]/1, [class.mem.general]/1, [dcl.decl.general]/1,
// [dcl.fct]/1: after a declarator, only GNU attributes may follow its asm
// label; a C++11 attribute-specifier-seq after them (or after a member's
// virt-specifiers or bit-width) has no place in the grammar. As clang 19:
// in a member-declarator it is reported ("an attribute list cannot appear
// here") and skipped, the member is declared; at namespace scope a first
// function declarator reports a missing function body and its declaration
// is skipped, other declarators a missing `;`. A `[[...]]` or `alignas`
// right after the parameter clause is the function type's.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 29:34 error "an attribute list cannot appear here"
//@ diag: 30:38 error "an attribute list cannot appear here"
//@ diag: 31:39 error "an attribute list cannot appear here"
//@ diag: 32:37 error "an attribute list cannot appear here"
//@ diag: 33:28 error "an attribute list cannot appear here"
//@ diag: 33:15 error "'v' marked 'override' but does not override any member functions"
//@ diag: 34:13 error "an attribute list cannot appear here"
//@ diag: 40:33 error "expected function body after function declarator"
//@ diag: 41:11 error "'alignas' attribute cannot be applied to types"
//@ diag: 42:35 error "expected ';' after top level declarator"
//@ diag: 43:35 error "expected ';' after top level declarator"
//@ diag: 44:26 error "expected ';' after top level declarator"
//@ diag: 47:15 error "use of undeclared identifier 'nf'"
//@ diag: 49:34 error "expected ';' at end of declaration"
//@ diag: 50:33 error "expected ';' at end of declaration"
struct A {
  void f() __attribute__((cold)) [[nodiscard]];
  int g() [[]] __attribute__((cold)) [[deprecated]] [[maybe_unused]];
  int h() const __attribute__((cold)) alignas(8);
  int x __attribute__((aligned(8))) [[maybe_unused]];
  virtual int v() override [[nodiscard]];
  int w : 3 [[maybe_unused]];
  int k() [[]] __attribute__((cold));
};
// The member declarations survive the skipped attribute lists.
int use(A &a) { return a.g() + a.h() + a.x + a.k(); }

void nf() __attribute__((cold)) [[nodiscard]];
void ng() alignas(8);
int ny __attribute__((aligned(8))) [[maybe_unused]];
int nz, nh() __attribute__((cold)) [[nodiscard]];
void nk() asm("nk_label") [[nodiscard]];
void nl() [[]] __attribute__((cold));
// `nf` was not declared (its declaration was skipped); `ny` was.
int after() { nf(); nl(); return ny; }
void block() {
  void bf() __attribute__((cold)) [[nodiscard]];
  int bx __attribute__((unused)) [[maybe_unused]];
}
