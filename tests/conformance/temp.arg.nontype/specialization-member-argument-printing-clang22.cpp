// [temp.arg.nontype], [temp.inst] (WP-1258): a substituted pointer to
// a member of a class nested in a class template specialization, in a
// static_assert requirement, is qualified by the class with its full
// scope (`no<&ns::R<int>::Q::w>`), as clang 22 prints the qualifier
// `BuildExpressionFromDeclTemplateArgument` makes; clang 19 printed the
// class alone (`no<&Q::w>`). See specialization-member-argument-printing.cpp.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 16:44 error "static assertion failed due to requirement 'no<&ns::R<int>::Q::w>': no"
//@ diag: 17:22 note "in instantiation of template class 'X<&ns::R<int>::Q::w>' requested here"
namespace ns {
template <class T> struct R { struct Q { int w; }; };
}
template <auto V> constexpr bool no = false;
template <auto V> struct X { static_assert(no<V>, "no"); };
X<&ns::R<int>::Q::w> e;
