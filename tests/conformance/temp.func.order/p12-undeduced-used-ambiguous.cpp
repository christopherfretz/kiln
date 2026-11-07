// [temp.deduct.partial]/12, [over.match.best]/2.5: when each template
// names, in a non-deduced context, a parameter the other's types cannot
// deduce, neither is at least as specialized and the call is ambiguous,
// for a type and a non-type parameter alike.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:3 error "call to 'f' is ambiguous"
//@ diag: 16:33 note "candidate function [with T = int, O = It]"
//@ diag: 17:33 note "candidate function [with T = int, O = It]"
//@ diag: 22:3 error "call to 'fn' is ambiguous"
//@ diag: 18:31 note "candidate function [with N = 1, O = It]"
//@ diag: 19:31 note "candidate function [with N = 1, O = It]"
struct It {};
template <class T> struct SelT { using type = It; };
template <int N> struct SelN { using type = It; };
template <class T, class O> int f(typename SelT<T>::type, O);
template <class T, class O> int f(O, typename SelT<T>::type);
template <int N, class O> int fn(typename SelN<N>::type, O);
template <int N, class O> int fn(O, typename SelN<N>::type);
void use(It it) {
  f<int>(it, it);
  fn<1>(it, it);
}
