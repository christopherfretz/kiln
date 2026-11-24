// [temp.friend], [temp.expl.spec]: a friend class or function template
// whose qualifier names a specialization that is not explicitly specialized
// (Bq<4>, or a dependent Bq<...> resolved at instantiation) names a member of
// a non-templated nested type, so its template-head must be 'template<>'.
// NS::W and a member of the explicit specialization Bq<9> are valid.
// Reduced from real code.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:3 error "template parameter list matching the non-templated nested type 'Bq<8>' should be empty ('template<>')"
//@ diag: 20:3 error "template parameter list matching the non-templated nested type 'Bq<8>' should be empty ('template<>')"
//@ diag: 23:3 error "template parameter list matching the non-templated nested type 'Bq<4>' should be empty ('template<>')"
//@ diag: 24:3 error "template parameter list matching the non-templated nested type 'Bq<4>' should be empty ('template<>')"
//@ diag: 25:3 error "template parameter list matching the non-templated nested type 'Bq<4>' should be empty ('template<>')"
template <int N> struct Bq { struct Z {}; template <int M> struct T {}; template <class U> void g(U); };
namespace NS { template <int M> struct W; }
template <> struct Bq<9> { template <int M> struct T {}; };
int f(...);
template <class T> struct S {
  template <int M> friend struct Bq<sizeof(sizeof(f(T())))>::Z;
  template <int M> friend struct Bq<sizeof(sizeof(f(T())))>::T;
};
struct X {
  template <int M> friend struct Bq<4>::Z;
  template <int M> friend struct Bq<4>::T;
  template <class U> friend void Bq<4>::g(U);
  template <int M> friend struct NS::W;
  template <int M> friend struct Bq<9>::T;
};
S<int> s;
