// [temp.res.general]/4: in a typename-specifier whose qualifier names no
// member, only the first failing component is diagnosed; the rest of the
// specifier is skipped (libc++ tuple_element_t).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:33 error "no member named 'type' in 'Holder<int>'"
//@ diag: 12:63 error "no member named 'type' in 'Holder<int>'"
//@ diag: 13:10 note "in instantiation of template class 'Use<int>' requested here"
template <class T> struct Holder {};
template <class T> using Elem = typename Holder<T>::type::value_type;
using E = typename Holder<int>::type::value_type;
template <class T> struct Use { using U = typename Holder<T>::type::template rebind<int>; };
Use<int> u;
