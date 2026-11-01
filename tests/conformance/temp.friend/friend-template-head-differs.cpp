// [temp.over.link]/6, [temp.friend]/1: the head of a friend class template declared in a class
// template must be equivalent to the namespace-scope template's once the enclosing class is
// instantiated (clang diagnoses it then, with the instantiation note); a namespace-scope
// redeclaration is compared with the namespace-scope declaration before an intervening friend.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:41 error "type constraint differs in template redeclaration"
//@ diag: 34:8 note "in instantiation of template class 'X<int>' requested here"
//@ diag: 22:11 note "previous template declaration is here"
//@ diag: 25:41 error "type constraint differs in template redeclaration"
//@ diag: 35:8 note "in instantiation of template class 'Y<int>' requested here"
//@ diag: 24:11 note "previous template declaration is here"
//@ diag: 27:59 error "requires clause differs in template redeclaration"
//@ diag: 36:8 note "in instantiation of template class 'Z<int>' requested here"
//@ diag: 26:29 note "previous template declaration is here"
//@ diag: 29:44 error "template non-type parameter has a different type 'long' in template redeclaration"
//@ diag: 37:10 note "in instantiation of template class 'NT<long>' requested here"
//@ diag: 28:15 note "previous non-type template parameter with type 'int' is here"
//@ diag: 33:11 error "type constraint differs in template redeclaration"
//@ diag: 30:11 note "previous template declaration is here"
template <class T, class U = T> concept C = __is_same(T, U);
template <C T> struct S;
template <class U> struct X { template <class T> friend struct S; };
template <C<long> T> struct Q;
template <class U> struct Y { template <C<U> T> friend struct Q; };
template <class T> requires (sizeof(T) > 1) struct R;
template <class U> struct Z { template <class T> requires (sizeof(T) > 2) friend struct R; };
template <int N> struct V;
template <class U> struct NT { template <U N> friend struct V; };
template <C T> struct W;
template <class U> struct A { template <C T> friend struct W; };
template <class T> concept E = true;
template <E T> struct W;
X<int> x;
Y<int> y;
Z<int> z;
NT<long> nt;
NT<int> ok;
