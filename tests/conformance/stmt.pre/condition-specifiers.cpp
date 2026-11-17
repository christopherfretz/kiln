//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0 -Wno-empty-body
//@ diag: 22:7 error "type name does not allow function specifier to be specified"
//@ diag: 23:7 error "type name does not allow function specifier to be specified"
//@ diag: 24:10 error "type name does not allow function specifier to be specified"
//@ diag: 25:11 error "type name does not allow function specifier to be specified"
//@ diag: 26:22 error "type name does not allow storage class to be specified"
//@ diag: 26:15 error "type name does not allow function specifier to be specified"
//@ diag: 26:7 error "type name does not allow function specifier to be specified"
//@ diag: 27:10 error "type name does not allow function specifier to be specified"
//@ diag: 29:7 error "'virtual' can only appear on non-static member functions"
// [stmt.pre]: each decl-specifier of a condition's decl-specifier-seq
// is a type-specifier or `constexpr`; a function specifier is reported
// by the type-specifier-seq rule, at each specifier, in clang's order
// (storage class, then inline, virtual, explicit) and dropped (clang 22;
// GCC 16.1 says "'virtual' invalid in condition"). An init-statement is
// a simple-declaration: `virtual` there is the variable rule.
// Found running GCC's C++ testsuite.
void f() {
  if (virtual int x = 1) {}
  if (explicit int x = 1) {}
  while (inline int x = 0) {}
  switch (explicit(true) int x = 1) { default: break; }
  if (virtual inline static int x = 1) {}
  for (; inline int x = 0;) {}
  if (constexpr int x = 1) {}
  if (virtual int x = 1; x) {}
  if (const int x = 1; constexpr int y = 2) {}
}
