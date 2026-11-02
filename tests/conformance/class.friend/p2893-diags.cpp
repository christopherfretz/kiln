//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 20:34 error "friend declaration contains unexpanded parameter pack 'Ts'"
//@ diag: 21:38 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 23:19 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 25:21 error "a type specifier is required for all declarations"
//@ diag: 26:46 error "friends can only be classes or functions"
//@ diag: 26:48 error "expected ';' at end of declaration list"
//@ diag: 27:22 error "expected ';' after friend declaration statement"
//@ diag: 28:46 error "friend declaration contains unexpanded parameter pack 'Ts'"
// [temp.variadic]/12: a friend naming a pack outside an expansion
// (a type or a function's type);
// [temp.variadic]/5.6: `...` after a friend-type-specifier that names no
// pack; [class.friend]/3 (C++26 P2893): every element of a
// friend-type-specifier-list is a type (`f` is a function), only types
// can be expanded (`friend void Ts...` declares a function), and the list
// ends with `;`.
struct A; struct B; void f();
template <class... Ts> class E { friend Ts; };
template <class T> class F { friend T...; };
template <class... Ts> class G { friend Ts...; };
class H { friend A..., B; };
G<A, B> g;
class I { friend A, f; };
template <class... Ts> class K { friend void Ts...; };
class L { friend A, B };
template <class... Ts> class M { friend void m(Ts); };
