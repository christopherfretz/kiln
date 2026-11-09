// [class.static.data]/4: an out-of-line definition without an initializer
// of a member initialized in the class, and an initializer at the
// definition of one that has none in the class, are valid (WP-1015).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
struct S { static const int i = 1; static const int j; };
const int S::i;
const int S::j = 2;
template<class T> struct E { static const int i = 1; static const int j; };
template<class T> const int E<T>::i;
template<class T> const int E<T>::j = 3;
int e = E<int>::i + E<int>::j + S::i + S::j;
static_assert(S::i == 1 && E<long>::i == 1);
