// [except.handle]/4 (WP-160): clang's `CatchHandlerType` drops the handler
// type's top-level cv-qualifiers before stripping a reference or pointer, so
// the referred-to type keeps its own: `int` then `const int &` is not a
// duplicate, `const int` then `int &` is. A reference or pointer to a public
// base matches regardless of cv-qualification.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 37:38 warning "exception of type 'int &' will be caught by earlier handler"
//@ diag: 37:23 note "for type 'const int'"
//@ diag: 44:32 warning "exception of type 'int' will be caught by earlier handler"
//@ diag: 44:17 note "for type 'int'"
//@ diag: 47:38 warning "exception of type 'D &' will be caught by earlier handler"
//@ diag: 47:23 note "for type 'const B &'"
//@ diag: 48:38 warning "exception of type 'const D &' will be caught by earlier handler"
//@ diag: 48:17 note "for type 'B &'"
//@ diag: 49:30 warning "exception of type 'D' will be caught by earlier handler"
//@ diag: 49:17 note "for type 'B'"
//@ diag: 50:36 warning "exception of type 'D' will be caught by earlier handler"
//@ diag: 50:23 note "for type 'const B'"
//@ diag: 51:32 warning "exception of type 'D *' will be caught by earlier handler"
//@ diag: 51:17 note "for type 'B *'"
//@ diag: 52:38 warning "exception of type 'D *' will be caught by earlier handler"
//@ diag: 52:23 note "for type 'const B *'"
//@ diag: 53:38 warning "exception of type 'const D *' will be caught by earlier handler"
//@ diag: 53:17 note "for type 'B *'"
//@ diag: 54:32 warning "exception of type 'E &' will be caught by earlier handler"
//@ diag: 54:17 note "for type 'B &'"
//@ diag: 55:47 warning "exception of type 'E &' will be caught by earlier handler"
//@ diag: 55:17 note "for type 'D &'"
//@ diag: 56:47 warning "exception of type 'D &' will be caught by earlier handler"
//@ diag: 56:32 note "for type 'const volatile B &'"
struct B {};
struct D : B {};
struct E : D {};
void f1() {
  try {} catch (int) {} catch (const int &) {}
  try {} catch (const int) {} catch (int &) {}
  try {} catch (int &) {} catch (const int &) {}
  try {} catch (const int &) {} catch (int &) {}
  try {} catch (int *) {} catch (const int *) {}
  try {} catch (B) {} catch (const B &) {}
  try {} catch (const B &) {} catch (B) {}
  try {} catch (B&) {} catch (const B &) {}
  try {} catch (int) {} catch (int) {}
}
void f2() {
  try {} catch (const B &) {} catch (D &) {}
  try {} catch (B &) {} catch (const D &) {}
  try {} catch (B) {} catch (D) {}
  try {} catch (const B) {} catch (D) {}
  try {} catch (B *) {} catch (D *) {}
  try {} catch (const B *) {} catch (D *) {}
  try {} catch (B *) {} catch (const D *) {}
  try {} catch (B &) {} catch (E &) {}
  try {} catch (D &) {} catch (B &) {} catch (E &) {}
  try {} catch (const volatile B &) {} catch (D &) {}
}
