// [basic.stc.dynamic.allocation]/1, [basic.stc.dynamic.deallocation]/2: the
// first parameter of an allocation function is std::size_t, of a
// deallocation function void*, and not a dependent type (clang 22
// wording; -19 sibling). WP-1015.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 17:18 error "'operator new' takes type size_t ('unsigned long') as 1st parameter"
//@ diag: 18:39 error "'operator new' cannot take a dependent type as its 1st parameter; use size_t ('unsigned long') instead"
//@ diag: 19:17 error "1st parameter of 'operator delete' must have type 'void *'"
//@ diag: 20:38 error "'operator delete' cannot take a dependent type as its 1st parameter; use 'void *' instead"
//@ diag: 21:36 error "'operator new' cannot take a dependent type as its 1st parameter; use size_t ('unsigned long') instead"
//@ diag: 21:58 error "'operator delete' cannot take a dependent type as its 1st parameter; use 'void *' instead"
//@ diag: 22:6 error "1st parameter of 'operator delete' must have type 'void *'"
typedef decltype(sizeof(0)) size_t;
struct D { void *operator new(bool); };
struct H { template<typename T> void *operator new(T, int); };
struct K { void operator delete(int*); };
struct M { template<typename T> void operator delete(T*, int); };
template<class T> struct Q { void* operator new(T); void operator delete(T); };
void operator delete(int);
