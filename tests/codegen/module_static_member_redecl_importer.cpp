// [depr.static.constexpr], [basic.def]/2.3, [module.import] (WP-1523):
// the redundant redeclaration `constexpr int A::r;` in the interface
// unit makes the member `weak_odr` there, but it is no definition and
// the member's initialization does nothing at run time, so the module's
// initializer does nothing and the importer neither calls it nor emits
// a `_GLOBAL__sub_I`; the importer exposes the member's value as an
// `available_externally` constant (clang 22 and 19). A member defined
// out of class without an initializer stays an `external` declaration.
//@ module: mod_static_member_redecl=Inputs/mod_static_member_redecl.cppm
import mod_static_member_redecl;
int use() { return *&A::n + *&A::r + *&A::m + *&A::i; }
