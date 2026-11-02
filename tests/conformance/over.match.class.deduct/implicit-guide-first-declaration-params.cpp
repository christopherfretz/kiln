// [over.match.class.deduct]/1.1, [temp.class.general]: the implicit guides of
// a class template name the injected class type with the template parameters
// of its first declaration (a forward declaration's `_Alloc`, unnamed
// parameters as `type-parameter-0-N`). A friend declaration in a non-template
// class counts as the first declaration; one in a class template does not.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:8 error "no viable constructor or deduction guide for deduction of template arguments of 'vector'"
//@ diag: 32:62 note "candidate template ignored: could not match 'vector<_Tp, _Alloc>' against 'int'"
//@ diag: 32:62 note "implicit deduction guide declared as 'template <class _Tp, class _Allocator = al<_Tp>> vector(vector<_Tp, _Alloc>) -> vector<_Tp, _Alloc>'"
//@ diag: 32:79 note "candidate function template not viable: requires 3 arguments, but 1 was provided"
//@ diag: 32:79 note "implicit deduction guide declared as 'template <class _Tp, class _Allocator = al<_Tp>> vector(int, int, int) -> vector<_Tp, _Alloc>'"
//@ diag: 36:4 error "no viable constructor or deduction guide for deduction of template arguments of 'mp'"
//@ diag: 34:35 note "candidate template ignored: could not match 'mp<type-parameter-0-0, type-parameter-0-1>' against 'int'"
//@ diag: 34:35 note "implicit deduction guide declared as 'template <class K, class V> mp(mp<type-parameter-0-0, type-parameter-0-1>) -> mp<type-parameter-0-0, type-parameter-0-1>'"
//@ diag: 34:48 note "candidate function template not viable: requires 3 arguments, but 1 was provided"
//@ diag: 34:48 note "implicit deduction guide declared as 'template <class K, class V> mp(int, int, int) -> mp<type-parameter-0-0, type-parameter-0-1>'"
//@ diag: 41:5 error "no viable constructor or deduction guide for deduction of template arguments of 'vec'"
//@ diag: 39:35 note "candidate template ignored: could not match 'vec<A, B>' against 'int'"
//@ diag: 39:35 note "implicit deduction guide declared as 'template <class T, class U> vec(vec<A, B>) -> vec<A, B>'"
//@ diag: 39:49 note "candidate function template not viable: requires 3 arguments, but 1 was provided"
//@ diag: 39:49 note "implicit deduction guide declared as 'template <class T, class U> vec(int, int, int) -> vec<A, B>'"
//@ diag: 42:6 error "no viable constructor or deduction guide for deduction of template arguments of 'vec2'"
//@ diag: 40:35 note "candidate template ignored: could not match 'vec2<T, U>' against 'int'"
//@ diag: 40:35 note "implicit deduction guide declared as 'template <class T, class U> vec2(vec2<T, U>) -> vec2<T, U>'"
//@ diag: 40:50 note "candidate function template not viable: requires 3 arguments, but 1 was provided"
//@ diag: 40:50 note "implicit deduction guide declared as 'template <class T, class U> vec2(int, int, int) -> vec2<T, U>'"
//@ requires: clang<22
// clang 22 emits error: no viable constructor or deduction guide for deduction of template argument...; WP-355 residual
template <class T> struct al {};
template <class _Tp, class _Alloc = al<_Tp> > class vector;
template <class _Tp, class _Allocator /* = al<_Tp> */> class vector { public: vector(int, int, int); };
template <class, class> class mp;
template <class K, class V> class mp { public: mp(int, int, int); };
vector v(1);
mp m(1);
struct W { template <class A, class B> friend class vec; };
template <class X> struct W2 { template <class C, class D> friend class vec2; };
template <class T, class U> class vec { public: vec(int, int, int); };
template <class T, class U> class vec2 { public: vec2(int, int, int); };
vec vv(1);
vec2 v2(1);
