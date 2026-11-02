// [temp.alias]/2, [dcl.fct]/5: a function type formed by substitution
// adjusts its parameter types (array and function to pointer, top-level cv
// dropped), and a parameter of type void after substitution is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:19 error "argument may not have 'void' type"
//@ diag: 32:1 note "in instantiation of template type alias 'F' requested here"
//@ diag: 23:22 error "argument may not have 'void' type"
//@ diag: 33:1 note "in instantiation of template type alias 'G' requested here"
//@ diag: 34:38 error "argument may not have 'void' type"
//@ diag: 35:9 note "in instantiation of template class 'W<void>' requested here"
//@ diag: 38:17 error "no function template matches function template specialization 'k'"
//@ diag: 37:24 note "candidate template ignored: substitution failure [with T = void]: argument may not have 'void' type"
template<class A, class B> struct same { static constexpr bool v = false; };
template<class A> struct same<A, A> { static constexpr bool v = true; };

template<class T> using F = void(T);
static_assert(same<F<int[3]>, void(int*)>::v);
static_assert(same<F<const int[3]>, void(const int*)>::v);
static_assert(same<F<int()>, void(int(*)())>::v);
static_assert(same<F<const int>, void(int)>::v);

template<class... T> using G = void(T...);
static_assert(same<G<int[2], char()>, void(int*, char(*)())>::v);

template<class T> using P = void(*)(T, T[]);
static_assert(same<P<int[2]>, void(*)(int*, int(*)[2])>::v);

template<class T> void m(T);
static_assert(same<decltype(m<int[3]>), void(int*)>::v);

F<void> *fv;
G<int, void> *gv;
template<class T> struct W { void f(T); };
W<void> w;

template<class T> void k(T t);
template<> void k<void>();
