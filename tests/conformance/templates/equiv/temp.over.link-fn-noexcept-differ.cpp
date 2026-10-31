//@ expect: compile-fail
//@ diag: 6:25 error "exception specification in declaration does not match previous declaration"
//@ diag: 5:25 note "previous declaration is here"
// [except.spec]/5: noexcept(sizeof(T) > 1) and noexcept(sizeof(T) > 2).
template <class T> void g(T) noexcept(sizeof(T) > 1);
template <class T> void g(T) noexcept(sizeof(T) > 2) {}
