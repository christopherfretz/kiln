// [temp.param]/2: `typename` must begin a type-parameter. After a
// failed `typename` (a decl-specifier before it) the following name is
// not consumed: the next diagnostic is at it, and no extra
// "expected ';'" follows.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:27 error "expected a qualified name after 'typename'"
//@ diag: 20:11 error "invalid declaration specifier in template non-type parameter"
//@ diag: 21:36 error "expected a qualified name after 'typename'"
//@ diag: 21:20 error "invalid declaration specifier in template non-type parameter"
//@ diag: 22:20 error "expected a qualified name after 'typename'"
//@ diag: 22:20 error "expected ',' or '>' in template-parameter-list"
//@ diag: 23:11 error "invalid declaration specifier in template non-type parameter"
//@ diag: 23:25 error "non-type template parameter has incomplete type 'class U'"
//@ diag: 23:24 note "forward declaration of 'U'"
//@ diag: 24:27 error "expected a qualified name after 'typename'"
//@ diag: 24:11 error "invalid declaration specifier in template non-type parameter"
//@ diag: 24:28 error "non-type template parameter has incomplete type 'U'"
//@ diag: 23:24 note "forward declaration of 'U'"
template <static typename U> struct A1 {};
template <class T, static typename U> struct A2 {};
template <typename static U> struct A3 {};
template <static class U> struct A4 {};
template <inline typename U, class V> struct A5 {};
