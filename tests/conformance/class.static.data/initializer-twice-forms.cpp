// [class.static.data]/4: when the in-class declaration of a static data
// member has an initializer, the definition has none; reported at the new
// initializer's expression location with a note at the in-class one. A
// constexpr or inline member is defined in the class: that is a
// redefinition (WP-1015).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:39 error "static data member 'i' already has an initializer"
//@ diag: 21:51 note "previous initialization is here"
//@ diag: 24:15 error "static data member 'i' already has an initializer"
//@ diag: 23:30 note "previous initialization is here"
//@ diag: 26:15 error "static data member 'i' already has an initializer"
//@ diag: 25:33 note "previous initialization is here"
//@ diag: 28:20 error "static data member 'i' already has an initializer"
//@ diag: 27:33 note "previous initialization is here"
//@ diag: 30:14 error "redefinition of 'i'"
//@ diag: 29:33 note "previous definition is here"
//@ diag: 32:8 error "redefinition of 'i'"
//@ diag: 31:30 note "previous definition is here"
template<class T> struct E { static const int i = 1; };
template<class T> const int E<T>::i = 2;
struct F { static const int i{1}; };
const int F::i{2};
struct G { static const int i = 1; };
const int G::i(2);
struct H { static const int i = 1; };
const int H::i = 1 + 1;
struct B { static constexpr int i = 1; };
const int B::i = 2;
struct D { inline static int i = 1; };
int D::i = 2;
