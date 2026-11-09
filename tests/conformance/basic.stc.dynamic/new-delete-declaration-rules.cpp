// [basic.stc.dynamic.allocation]/1, [basic.stc.dynamic.deallocation]/1-2:
// an allocation or deallocation function is a class member or global, not
// static at global scope; it returns void* (void), not a dependent type; a
// template one has at least two parameters; there is at least one
// parameter; an allocation function's size parameter has no default
// argument. WP-1015 (clang's CheckOperatorNewDeleteDeclaration).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:9 error "'operator new' cannot be declared inside a namespace"
//@ diag: 34:8 error "'operator delete' cannot be declared inside a namespace"
//@ diag: 35:9 error "'operator new[]' cannot be declared inside a namespace"
//@ diag: 37:14 error "static declaration of 'operator new' follows non-static declaration"
//@ diag: 37:14 note "previous implicit declaration is here"
//@ diag: 38:14 error "'operator new' cannot be declared static in global scope"
//@ diag: 39:13 error "static declaration of 'operator delete' follows non-static declaration"
//@ diag: 39:13 note "previous implicit declaration is here"
//@ diag: 40:13 error "'operator delete' cannot be declared static in global scope"
//@ diag: 41:17 error "'operator new' must return type 'void *'"
//@ diag: 42:18 error "'operator new' must have at least one parameter"
//@ diag: 43:18 error "parameter of 'operator new' cannot have a default argument"
//@ diag: 44:35 error "'operator new' cannot have a dependent return type; use 'void *' instead"
//@ diag: 45:39 error "'operator new' template must have at least two parameters"
//@ diag: 46:16 error "'operator delete' must return type 'void'"
//@ diag: 47:17 error "'operator delete' must have at least one parameter"
//@ diag: 48:35 error "'operator delete' cannot have a dependent return type; use 'void' instead"
//@ diag: 49:38 error "'operator delete' template must have at least two parameters"
//@ diag: 50:32 error "'operator new' cannot have a dependent return type; use 'void *' instead"
//@ diag: 51:5 error "'operator new' must return type 'void *'"
//@ diag: 52:6 error "'operator delete' must have at least one parameter"
typedef decltype(sizeof(0)) size_t;
namespace NS {
  void *operator new(size_t);
  void operator delete(void *);
  void *operator new[](size_t, int);
}
static void *operator new(size_t);
static void *operator new(size_t, int, int);
static void operator delete(void *);
static void operator delete(void *, int, int);
struct B { void operator new(size_t); };
struct C { void *operator new(); };
struct E { void *operator new(size_t = 0); };
struct G { template<typename T> T operator new(size_t, int); };
struct I { template<typename T> void *operator new(size_t); };
struct J { int operator delete(void*); };
struct L { void operator delete(); };
struct N { template<typename T> T operator delete(void*, int); };
struct O { template<typename T> void operator delete(void*); };
template<class T> struct R { T operator new(size_t); };
int operator new(size_t, char);
void operator delete();
