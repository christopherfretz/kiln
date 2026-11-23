// [temp.explicit]/3-4, [temp.names]/3, [dcl.type.elab]/5 (WP-2547): an
// explicit instantiation whose name is followed by `<` but names no
// template. A name nothing declares, and no template name resembles, is an
// "undeclared template"; a name that finds a class is a "non-template". The
// declaration then recovers as the elaborated-type-specifier `struct w;`, as
// clang's does, so `w` is the incomplete class `b::w` afterwards and a later
// class-key is checked against it. A qualified name finds a non-template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:17 error "explicit instantiation of undeclared template struct 'w'"
//@ diag: 28:23 error "explicit instantiation of non-template class 'w'"
//@ diag: 29:16 error "explicit instantiation of non-template union 'w'"
//@ diag: 29:10 error "use of 'w' with tag type that does not match previous declaration"
//@ diag: 28:23 note "previous use is here"
//@ diag: 30:3 error "variable has incomplete type 'w'"
//@ diag: 27:17 note "forward declaration of 'b::w'"
//@ diag: 34:24 error "explicit instantiation of undeclared template struct 'u'"
//@ diag: 38:20 error "explicit instantiation of non-template struct 'S'"
//@ diag: 38:17 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 39:27 error "explicit instantiation of non-template struct 'S'"
//@ diag: 39:24 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 41:17 error "explicit instantiation of non-template struct 'T'"
//@ diag: 42:23 error "explicit instantiation of non-template class 'T'"
//@ diag: 43:17 error "explicit instantiation of non-template struct 'T'"
namespace b {
struct M {};
template struct w<M>;
extern template class w<M>;
template union w<M>;
w z;
}
namespace c {
struct M {};
extern template struct u<M>;
u *p;
}
namespace N { struct S {}; }
template struct N::S<int>;
extern template struct N::S<int>;
struct T {};
template struct T<int>;
extern template class T<int>;
template struct T<int> x;
