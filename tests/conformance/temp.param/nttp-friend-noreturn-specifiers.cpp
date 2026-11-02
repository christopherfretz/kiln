// [temp.param]/2, [class.friend]/1, [dcl.fct.spec]/1: the decl-specifier
// leftovers of a non-type template parameter, as clang. `friend` outside a
// class is reported while parsing ("'friend' used outside of class") and
// dropped, so the parameter stays usable; C11 `_Noreturn` (a clang
// extension in C++) is one more "invalid declaration specifier", after
// the others in clang's `CheckValidDeclSpecifiers` order. `static class T`
// declares a non-type parameter of the incomplete class type `T`.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 31:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 31:24 error "non-type template parameter has incomplete type 'class T'"
//@ diag: 31:23 note "forward declaration of 'T'"
//@ diag: 31:39 error "field has incomplete type 'T'"
//@ diag: 31:23 note "forward declaration of 'T'"
//@ diag: 32:3 error "template argument for non-type template parameter must be an expression"
//@ diag: 31:24 note "template parameter is declared here"
//@ diag: 33:10 error "'friend' used outside of class"
//@ diag: 35:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 37:17 error "'friend' used outside of class"
//@ diag: 37:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 39:10 error "'friend' used outside of class"
//@ diag: 39:24 error "non-type template parameter has incomplete type 'class V'"
//@ diag: 39:23 note "forward declaration of 'V'"
//@ diag: 39:39 error "field has incomplete type 'V'"
//@ diag: 39:23 note "forward declaration of 'V'"
//@ diag: 40:3 error "template argument for non-type template parameter must be an expression"
//@ diag: 39:24 note "template parameter is declared here"
//@ diag: 41:20 warning "duplicate '_Noreturn' declaration specifier"
//@ diag: 41:10 error "invalid declaration specifier in template non-type parameter"
template<static class T> struct A { T t; };
A<int> a;
template<friend int N> struct B { static const int v = N; };
static_assert(B<3>::v == 3);
template<_Noreturn int N> struct C { static const int v = N; };
static_assert(C<4>::v == 4);
template<static friend int M> struct D { static const int v = M; };
static_assert(D<5>::v == 5);
template<friend class V> struct G { V v; };
G<int> g;
template<_Noreturn _Noreturn int K> struct H {};
