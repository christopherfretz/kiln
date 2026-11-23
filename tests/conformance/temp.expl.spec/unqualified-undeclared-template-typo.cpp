// [temp.expl.spec]/2-3, [temp.names]/3, [namespace.memdef] (WP-2547): in an
// explicit specialization whose unqualified name is followed by `<` and that
// ordinary lookup does not find, the class template the name was probably
// meant to be is suggested and specialized, so the specialization must be in
// a namespace enclosing it; a name no template resembles is an "undeclared
// template", one that finds a class a "non-template". The same suggestion is
// made for an elaborated-type-specifier, a partial specialization and a
// friend that name an undeclared `v<...>`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:20 error "no template named 'v'; did you mean 'a::v'?"
//@ diag: 34:41 note "'a::v' declared here"
//@ diag: 37:20 error "class template specialization of 'v' not in a namespace enclosing 'a'"
//@ diag: 34:41 note "explicitly specialized declaration is here"
//@ diag: 38:20 error "explicit specialization of undeclared template struct 'w'"
//@ diag: 40:20 error "explicit specialization of non-template struct 'S'"
//@ diag: 40:20 error "redefinition of 'S'"
//@ diag: 39:8 note "previous definition is here"
//@ diag: 41:17 error "explicit instantiation of non-template struct 'S'"
//@ diag: 42:23 error "class template specialization of 'v' not in a namespace enclosing 'a'"
//@ diag: 34:41 note "explicitly specialized declaration is here"
//@ diag: 45:8 error "no template named 'v'; did you mean 'a::v'?"
//@ diag: 34:41 note "'a::v' declared here"
//@ diag: 46:27 error "no template named 'v'; did you mean 'a::v'?"
//@ diag: 34:41 note "'a::v' declared here"
//@ diag: 46:27 error "class template partial specialization of 'v' not in a namespace enclosing 'a'"
//@ diag: 34:41 note "explicitly specialized declaration is here"
//@ diag: 47:8 error "explicit specialization of undeclared template struct 'w'"
//@ diag: 48:27 error "explicit specialization of non-template struct 'w'"
//@ diag: 48:27 error "redefinition of 'w' as different kind of symbol"
//@ diag: 47:8 note "previous definition is here"
//@ diag: 49:26 error "no template named 'v'; did you mean 'a::v'?"
//@ diag: 34:41 note "'a::v' declared here"
namespace a { template <class T> struct v {}; }
namespace b {
struct M {};
template <> struct v<M> {};
template <> struct w<M> {};
struct S {};
template <> struct S<M> {};
template struct S<M>;
template <> struct a::v<int> {};
}
namespace c {
struct v<int> *p;
template <class T> struct v<T *> {};
struct w<int> *q;
template <class T> struct w<T *> {};
struct S { friend struct v<char>; };
}
