// [temp.inst]/4, /11, [temp.spec.general]: instantiating a class template
// specialization instantiates the declarations of its member function
// templates, so a member template whose parameter type, return type or
// pack-indexed parameter cannot be formed for P<> is an error during the
// class's instantiation: the note chain is "in instantiation of template
// class 'P<>' requested here" then the enclosing function template or
// member function. A default argument is instantiated only when used: its
// chain starts at "in instantiation of default function argument
// expression". The later "no member named" errors are clang's recovery.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:38 error "too many template arguments for class template 'A'"
//@ diag: 37:33 note "in instantiation of template class 'P<>' requested here"
//@ diag: 38:12 note "in instantiation of function template specialization 'u<>' requested here"
//@ diag: 34:29 note "template is declared here"
//@ diag: 37:42 error "no member named 'f' in 'P<>'"
//@ diag: 38:12 note "in instantiation of function template specialization 'u<>' requested here"
//@ diag: 41:31 error "too many template arguments for class template 'A'"
//@ diag: 43:44 note "in instantiation of template class 'Q<>' requested here"
//@ diag: 44:19 note "in instantiation of member function 'W<>::m' requested here"
//@ diag: 40:29 note "template is declared here"
//@ diag: 43:53 error "no member named 'g' in 'Q<>'"
//@ diag: 44:19 note "in instantiation of member function 'W<>::m' requested here"
//@ diag: 47:51 error "too many template arguments for class template 'A'"
//@ diag: 49:33 note "in instantiation of default function argument expression for 'h<'a', 'b'>' required here"
//@ diag: 50:13 note "in instantiation of function template specialization 'x<>' requested here"
//@ diag: 46:29 note "template is declared here"
//@ diag: 53:46 error "invalid index 0 for pack 'Ts' of size 0"
//@ diag: 55:34 note "in instantiation of template class 'P2<>' requested here"
//@ diag: 56:13 note "in instantiation of function template specialization 'u2<>' requested here"
//@ diag: 55:44 error "no member named 'f' in 'P2<>'"
//@ diag: 56:13 note "in instantiation of function template specialization 'u2<>' requested here"
template <class... Ts> struct P {
  template <Ts... a> struct A {};
  template <char... c> static void f(A<c...>);
};
template<class... T> void u() { P<T...>::f(); }
void t() { u<>(); }
template <class... Ts> struct Q {
  template <Ts... a> struct A {};
  template <char... c> static A<c...> g();
};
template<class... T> struct W { void m() { Q<T...>::g(); } };
void t2() { W<>().m(); }
template <class... Ts> struct R {
  template <Ts... a> struct A {};
  template <char... c> static void h(int = sizeof(A<c...>));
};
template<class... T> void x() { R<int, T...>::template h<'a', 'b'>(); }
void t3() { x<>(); }
template <class... Ts> struct P2 {
  template <class... Us> struct A {};
  template <char... c> static void f(A<Ts...[0]>);
};
template<class... T> void u2() { P2<T...>::f(); }
void t4() { u2<>(); }
