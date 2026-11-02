// [over.match.class.deduct]/1.1, [temp.class.general]: the implicit guides of
// a class template name the injected class type with the template parameters
// of its first declaration (a forward declaration's `_Alloc`, unnamed
// parameters as `type-parameter-0-N`). A friend declaration in a non-template
// class counts as the first declaration; one in a class template does not.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:8 error "no viable constructor or deduction guide for deduction of template arguments of 'vector'"
//@ diag: 30:62 note "candidate template ignored: could not match 'vector<_Tp, _Alloc>' against 'int'"
//@ diag: 30:62 note "implicit deduction guide declared as 'template <class _Tp, class _Allocator = al<_Tp>> vector(vector<_Tp, _Alloc>) -> vector<_Tp, _Alloc>'"
//@ diag: 30:79 note "candidate function template not viable: requires 3 arguments, but 1 was provided"
//@ diag: 30:79 note "implicit deduction guide declared as 'template <class _Tp, class _Allocator = al<_Tp>> vector(int, int, int) -> vector<_Tp, _Alloc>'"
//@ diag: 34:4 error "no viable constructor or deduction guide for deduction of template arguments of 'mp'"
//@ diag: 32:35 note "candidate template ignored: could not match 'mp<type-parameter-0-0, type-parameter-0-1>' against 'int'"
//@ diag: 32:35 note "implicit deduction guide declared as 'template <class K, class V> mp(mp<type-parameter-0-0, type-parameter-0-1>) -> mp<type-parameter-0-0, type-parameter-0-1>'"
//@ diag: 32:48 note "candidate function template not viable: requires 3 arguments, but 1 was provided"
//@ diag: 32:48 note "implicit deduction guide declared as 'template <class K, class V> mp(int, int, int) -> mp<type-parameter-0-0, type-parameter-0-1>'"
//@ diag: 39:5 error "no viable constructor or deduction guide for deduction of template arguments of 'vec'"
//@ diag: 37:35 note "candidate template ignored: could not match 'vec<A, B>' against 'int'"
//@ diag: 37:35 note "implicit deduction guide declared as 'template <class T, class U> vec(vec<A, B>) -> vec<A, B>'"
//@ diag: 37:49 note "candidate function template not viable: requires 3 arguments, but 1 was provided"
//@ diag: 37:49 note "implicit deduction guide declared as 'template <class T, class U> vec(int, int, int) -> vec<A, B>'"
//@ diag: 40:6 error "no viable constructor or deduction guide for deduction of template arguments of 'vec2'"
//@ diag: 38:35 note "candidate template ignored: could not match 'vec2<T, U>' against 'int'"
//@ diag: 38:35 note "implicit deduction guide declared as 'template <class T, class U> vec2(vec2<T, U>) -> vec2<T, U>'"
//@ diag: 38:50 note "candidate function template not viable: requires 3 arguments, but 1 was provided"
//@ diag: 38:50 note "implicit deduction guide declared as 'template <class T, class U> vec2(int, int, int) -> vec2<T, U>'"
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
