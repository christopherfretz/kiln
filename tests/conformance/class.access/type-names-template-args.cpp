// [class.access]/4: a type name passed as a template argument is checked where
// it is written (derived classes reach protected members, friends private ones).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:28 error "'Priv' is a private member of 'A'"
//@ diag: 15:10 note "implicitly declared private here"
//@ diag: 24:36 error "'Priv' is a private member of 'A'"
//@ diag: 15:10 note "implicitly declared private here"
//@ diag: 30:8 error "'Prot' is a protected member of 'A'"
//@ diag: 17:10 note "declared protected here"
template<bool B, class T, class F> struct cond { using type = T; };
template<class T, class F> struct cond<false, T, F> { using type = F; };
template<bool B, class T, class F> using cond_t = typename cond<B, T, F>::type;
class A {
  struct Priv { };
protected:
  struct Prot { };
  friend struct Fr;
  template<class> friend struct FT;
};
using X1 = cond_t<true, A::Priv, int>;
struct D : A {
  using X2 = cond_t<true, A::Prot, int>;
  using X3 = cond_t<false, int, A::Priv>;
};
struct Fr { using X4 = cond_t<false, int, A::Priv>; };
template<class T> struct FT { using X5 = cond_t<true, typename T::Priv, int>; };
FT<A> ft;
template<class T> struct Box { };
Box<A::Prot> *bp;
