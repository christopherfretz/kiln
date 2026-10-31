//@ expect: compile-fail
//@ diag: 7:26 error "type alias template redefinition with different types ('int[sizeof(U) + 2]' vs 'int[sizeof(T) + 1]')"
//@ diag: 6:26 note "previous definition is here"
// [temp.over.link]/5, [dcl.typedef]/4: `sizeof(T) + 1` and
// `sizeof(U) + 2` are not equivalent.
template <class T> using A = int[sizeof(T) + 1];
template <class U> using A = int[sizeof(U) + 2];
