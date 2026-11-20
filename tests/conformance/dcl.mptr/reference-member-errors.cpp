// [dcl.mptr]/3: a pointer to member shall not point to a member of
// reference type: clang's "cannot form a pointer-to-member to member
// ... of reference type" at the `&` (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:10 error "cannot form a pointer-to-member to member 'r' of reference type 'int &'"
//@ diag: 11:25 error "cannot form a pointer-to-member to member 'rr' of reference type 'const S &&'"

struct S { int& r; const S&& rr; int i; };
auto p = &S::r;
int q = sizeof(decltype(&S::rr));
auto ok = &S::i;
