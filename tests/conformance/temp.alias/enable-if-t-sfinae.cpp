// [temp.alias]/2, [temp.deduct.general]/8: substitution into an alias
// template's arguments is in the immediate context, so a missing
// `enable_if<false>::type` behind an alias is a substitution failure in a
// default template argument, a return type, a parameter type and a nested
// alias; a viable sibling is chosen silently. With no candidate left the note
// follows clang 19 (spec/sema-templates.md §5.7.1): an alias named
// `enable_if_t` reports the requirement written as its first argument, any
// other alias "disabled by 'enable_if'" at the alias pattern's argument.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 43:12 error "no matching function for call to 'f1'"
//@ diag: 33:65 note "candidate template ignored: requirement 'is_ptr<int>::value' was not satisfied [with T = int]"
//@ diag: 43:19 error "no matching function for call to 'f2'"
//@ diag: 34:25 note "candidate template ignored: requirement 'is_ptr<int>::value' was not satisfied [with T = int]"
//@ diag: 43:26 error "no matching function for call to 'f3'"
//@ diag: 28:67 note "candidate template ignored: disabled by 'enable_if' [with T = int]"
//@ diag: 43:33 error "no matching function for call to 'f4'"
//@ diag: 36:42 note "candidate template ignored: requirement 'false' was not satisfied [with T = int]"
//@ diag: 43:40 error "no matching function for call to 'f5'"
//@ diag: 37:64 note "candidate template ignored: requirement 'is_ptr<int>::value' was not satisfied [with T = int]"
//@ diag: 43:47 error "no matching function for call to 'f6'"
//@ diag: 38:25 note "candidate template ignored: requirement 'is_ptr<int>::value' was not satisfied [with T = int]"
//@ diag: 43:54 error "no matching function for call to 'f7'"
//@ diag: 28:67 note "candidate template ignored: disabled by 'enable_if' [with T = int]"
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { typedef T type; };
template <bool B, class T = void> using enable_if_t = typename enable_if<B, T>::type;
template <bool B, class T = void> using my_t = typename enable_if<B, T>::type;
template <class T> struct is_ptr { static constexpr bool value = false; };
template <class T> struct is_ptr<T*> { static constexpr bool value = true; };
template <class T> using A = enable_if_t<is_ptr<T>::value, int>;
template <class T> using A2 = my_t<is_ptr<T>::value, int>;
template <class T, enable_if_t<is_ptr<T>::value, int> = 0> void f1(T);
template <class T> A<T> f2(T);
template <class T> A2<T> f3(T);
template <class T> enable_if_t<false, T> f4(T);
template <class T, class = enable_if_t<is_ptr<T>::value>> void f5(T);
template <class T> void f6(T, enable_if_t<is_ptr<T>::value>* = 0);
template <class T> my_t<is_ptr<T>::value, int> f7(T);
template <class T, enable_if_t<!is_ptr<T>::value, int> = 0> long h(T);
template <class T, enable_if_t<is_ptr<T>::value, int> = 0> int h(T);
static_assert(sizeof(h(1)) == sizeof(long));
void g() { f1(1); f2(1); f3(1); f4(1); f5(1); f6(1); f7(1); }
