// A template argument of a `bool`-based enumeration type is named by its
// enumerator in the instantiation note (clang TemplateArgument::print).
// Verified against clang 19.
//@ expect: compile-fail
//@ diag: 8:41 error "static assertion failed"
//@ diag: 9:11 note "in instantiation of template class 'X<B::Nay>' requested here"
enum class B : bool { Yay, Nay };
template <B V> struct X { static_assert(V == B::Yay, "no"); };
X<B::Nay> x;
