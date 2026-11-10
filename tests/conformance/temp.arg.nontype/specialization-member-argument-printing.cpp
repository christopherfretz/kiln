// [temp.arg.nontype], [temp.inst] (spec/reflection-repr.md §5, WP-1258): a
// template argument naming a member of a class template specialization
// prints the specialization with its arguments at every level of the
// member's scope, as clang prints it: `X<&ns::R<int>::y>` for a static
// data member, `X<ns::R<char>::e1>` for an enumerator, `X<&S<long>::m>`
// for a pointer to member; a substituted pointer to member in a
// static_assert requirement is qualified by its class (`no<&S<long>::m>`,
// clang's `BuildExpressionFromDeclTemplateArgument`), a pointer to a
// static member is not (`no<&y>`). The qualifier's own scope is in
// specialization-member-argument-printing-clang22.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:44 error "static assertion failed due to requirement 'no<&y>': no"
//@ diag: 31:19 note "in instantiation of template class 'X<&ns::R<int>::y>' requested here"
//@ diag: 30:44 error "static assertion failed due to requirement 'no<(ns::R<char>::E)0U>': no"
//@ diag: 32:20 note "in instantiation of template class 'X<ns::R<char>::e1>' requested here"
//@ diag: 30:44 error "static assertion failed due to requirement 'no<&S<long>::m>': no"
//@ diag: 33:16 note "in instantiation of template class 'X<&S<long>::m>' requested here"
//@ diag: 30:44 error "static assertion failed due to requirement 'no<&z>': no"
//@ diag: 34:24 note "in instantiation of template class 'X<&ns::R<short>::Q::z>' requested here"
namespace ns {
template <class T> struct R {
  static inline int y;
  enum E { e1 };
  struct Q { static inline int z; };
};
}
template <class T> struct S { int m; };
template <auto V> constexpr bool no = false;
template <auto V> struct X { static_assert(no<V>, "no"); };
X<&ns::R<int>::y> a;
X<ns::R<char>::e1> b;
X<&S<long>::m> c;
X<&ns::R<short>::Q::z> d;
