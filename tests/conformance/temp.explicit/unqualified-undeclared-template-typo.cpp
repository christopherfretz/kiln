// [temp.explicit]/4-5, [temp.names]/3, [dcl.type.elab]/5, [namespace.memdef]
// (WP-2547): in an explicit instantiation whose unqualified name is followed
// by `<` and that ordinary lookup does not find, the name is not a template
// (it was taken as a new class `b::v`). The class template the name was
// probably meant to be is suggested, and the explicit instantiation then
// names it, so it must appear in that template's namespace (the C++20
// [temp.explicit]/5 rule clang applies; kiln's C++23 acceptance of an
// unqualified name outside the namespace does not reach a corrected name).
// Nothing named `b::v` is declared, so the later `b::v` finds nothing
// (also checked on the AST by `explicit_instantiation_typo_declares_nothing`
// in crates/kiln_parse/tests/templ.rs). The class-key of an explicit
// instantiation must agree with the template's ([dcl.type.elab]/5): the
// union one is also a tag mismatch, in the template's namespace too.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:56 error "use of 'v' with tag type that does not match previous declaration"
//@ diag: 37:41 note "previous use is here"
//@ diag: 42:17 error "no template named 'v'; did you mean 'a::v'?"
//@ diag: 37:41 note "'a::v' declared here"
//@ diag: 42:17 error "explicit instantiation of 'a::v' must occur in namespace 'a'"
//@ diag: 37:41 note "explicit instantiation refers here"
//@ diag: 43:24 error "no template named 'v'; did you mean 'a::v'?"
//@ diag: 37:41 note "'a::v' declared here"
//@ diag: 43:24 error "explicit instantiation of 'a::v' must occur in namespace 'a'"
//@ diag: 37:41 note "explicit instantiation refers here"
//@ diag: 44:16 error "no template named 'v'; did you mean 'a::v'?"
//@ diag: 37:41 note "'a::v' declared here"
//@ diag: 44:16 error "explicit instantiation of 'a::v' must occur in namespace 'a'"
//@ diag: 37:41 note "explicit instantiation refers here"
//@ diag: 45:16 error "no template named 'v'; did you mean 'a::v'?"
//@ diag: 37:41 note "'a::v' declared here"
//@ diag: 45:10 error "use of 'v' with tag type that does not match previous declaration"
//@ diag: 37:41 note "previous use is here"
//@ diag: 45:16 error "explicit instantiation of 'a::v' must occur in namespace 'a'"
//@ diag: 37:41 note "explicit instantiation refers here"
//@ diag: 46:4 error "no type named 'v' in namespace 'b'"
namespace a { template <class T> struct v {}; template union v<char>; }
namespace b {
struct M {};
struct N {};
struct P {};
template struct v<M>;
extern template struct v<N>;
template class v<P>;
template union v<int>;
b::v *y;
}
