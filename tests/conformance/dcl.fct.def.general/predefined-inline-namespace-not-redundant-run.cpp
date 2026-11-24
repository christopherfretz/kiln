// [dcl.fct.def.general]/8 (`__PRETTY_FUNCTION__`, clang's
// PredefinedExpr::ComputeName), [namespace.def]/7: clang leaves an inline
// namespace out of a type's scope only where it is redundant
// (`NamespaceDecl::isRedundantInlineQualifierFor`): `n::e` for
// `n::v1::w::e`, but `m::v1::e` and `m::v2::e` where `m::e` would name
// both. kiln's printer of a bare AST (`AstNames::append_scope`) leaves
// out every inline namespace and prints `m::e` for both. Found while
// fixing the uncaught-exception note's spelling (WP-3408).
//@ expect: run
//@ xfail(kiln): WP-3480 follow-up, the bare-AST type printer drops an inline namespace that is not redundant
//@ stdout: void P() [T = m::v1::e]
//@ stdout: void P() [T = W<m::v2::e>]
//@ stdout: void P() [T = n::e]
extern "C" int puts(const char *);
namespace m { inline namespace v1 { struct e {}; } inline namespace v2 { struct e {}; } }
namespace n { inline namespace v1 { inline namespace w { struct e {}; } } }
template <class T> struct W {};
template <class T> void P() { puts(__PRETTY_FUNCTION__); }
int main() {
  P<m::v1::e>();
  P<W<m::v2::e>>();
  P<n::e>();
}
