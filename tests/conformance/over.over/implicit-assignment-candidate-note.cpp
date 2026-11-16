// [over.over]/1-5, [class.copy.assign]: taking the address of an implicitly
// declared assignment operator selects by the target type; when none
// matches, each candidate is noted as the implicit copy or move assignment
// operator.
//@ expect: compile-fail
//@ diag: 12:21 error "address of overloaded function 'operator=' does not match required type 'void (int)'"
//@ diag: 9:8 note "candidate function (the implicit copy assignment operator) has type mismatch at 1st parameter (expected 'int' but has 'const D &')"
//@ diag: 9:8 note "candidate function (the implicit move assignment operator) has type mismatch at 1st parameter (expected 'int' but has 'D &&')"
struct D { int x; };
D& (D::*ok_copy)(const D&) = &D::operator=;
D& (D::*ok_move)(D&&) = &D::operator=;
void (*bad)(int) = &D::operator=;
