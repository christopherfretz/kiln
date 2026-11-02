// [temp.alias]/2, [temp.inst]/1: outside a deduction a missing
// `enable_if<false>::type` is a hard error; clang 19 says that `enable_if`
// cannot disable this declaration (spec/sema-templates.md §5.7.1), at the
// alias pattern's argument through an alias and at the failed condition when
// written directly, with the instantiation notes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:74 error "no type named 'type' in 'enable_if<false, int>'; 'enable_if' cannot be used to disable this declaration"
//@ diag: 18:31 note "in instantiation of template type alias 'enable_if_t' requested here"
//@ diag: 21:8 note "in instantiation of template class 'C<int>' requested here"
//@ diag: 19:50 error "failed requirement 'is_ptr<int>::value'; 'enable_if' cannot be used to disable this declaration"
//@ diag: 22:8 note "in instantiation of template class 'D<int>' requested here"
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { typedef T type; };
template <bool B, class T = void> using enable_if_t = typename enable_if<B, T>::type;
template <class T> struct is_ptr { static constexpr bool value = false; };
template <class T> struct is_ptr<T*> { static constexpr bool value = true; };
template <class P> struct C { enable_if_t<is_ptr<P>::value, int> m; };
template <class P> struct D { typename enable_if<is_ptr<P>::value, int>::type m; };
C<int*> c1;
C<int> c2;
D<int> d;
