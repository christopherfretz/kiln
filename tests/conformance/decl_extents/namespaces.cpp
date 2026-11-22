// The declaration-extent table (spec/ast.md §19.4): every opening of a
// namespace is a row with its braces as its body; `namespace a::b::c {`
// gives each of the three namespaces the one brace pair, `b` and `c`
// qualified from `a`; inline and anonymous namespaces; a namespace
// alias.
//@ decl-extents: namespaces.extents
//@ std: c++20
namespace a {
int x;
}
namespace a {
int y;
}
namespace a::b::c {
int z;
}
namespace a::inline v2 {}
inline namespace v1 {
namespace {
int hidden;
}
} // namespace v1
namespace [[deprecated]] old {}
namespace al = a::b::c;
