//@ expect: compile-fail
//@ diag: 12:26 error "type alias template redefinition with different types ('U *'"
//@ diag: 11:26 note "previous definition is here"
// [temp.alias], [dcl.typedef]/4 (via [temp.decls.general]): an alias
// template may be redeclared with the same type (libc++ declares
// `__swap_result_t` in two headers); a different type is an error.
template <class T> using R = T &;
template <class U> using R = U &;
static_assert(__is_same(R<int>, int &));

template <class T> using P = T &;
template <class U> using P = U *;
