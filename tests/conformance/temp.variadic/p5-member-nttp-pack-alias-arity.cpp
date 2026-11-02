// [temp.param]/17, [temp.arg.general]/1: an alias member template of a
// nested member class of `Q<Ts...>` naming the member template
// `template<Ts... a> struct A` with a pack expansion of its own expanded
// pack: `A<u...>` takes one argument for `u...`, `A<1, u...>` is "too many
// template arguments for class template 'A'" when `Ts` has one element,
// whatever `u` holds. (clang 19 adds an alias instantiation note.)
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 24:34 error "too many template arguments for class template 'A'"
//@ diag: 29:21 note "in instantiation of template class 'Q<int>::R<int, int>' requested here"
//@ diag: 21:29 note "template is declared here"
//@ diag: 24:34 error "too many template arguments for class template 'A'"
//@ diag: 30:13 note "in instantiation of template class 'Q<int>::R<>' requested here"
//@ diag: 21:29 note "template is declared here"
//@ diag: 24:34 error "too many template arguments for class template 'A'"
//@ diag: 31:16 note "in instantiation of template class 'Q<int>::R<int>' requested here"
//@ diag: 21:29 note "template is declared here"
template <class... Ts> struct Q {
  template <Ts... a> struct A {};
  template <class... Us> struct R {
    template <Us... u> using U = A<u...>;
    template <Us... u> using V = A<1, u...>;
  };
};
Q<int, char>::R<int, char> q1;
Q<int, char>::R<int> q2;
Q<int>::R<int, int> q3;
Q<int>::R<> q4;
Q<int>::R<int> q5;
