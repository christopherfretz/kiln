// [temp.variadic]/5, [temp.variadic]/12: a requires-expression whose
// parameter types or requirements name a pack is not expanded on its
// own, so outside a pack expansion it is an unexpanded pack, diagnosed
// at the enclosing full-expression as clang does; a fold whose pattern
// names no pack is an error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:21 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 19:21 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 21:27 error "static assertion contains unexpanded parameter pack 'Ts'"
//@ diag: 23:48 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 27:21 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 29:27 error "static assertion contains unexpanded parameter pack 'Ts'"
//@ diag: 30:48 error "static assertion contains unexpanded parameter pack 'Ts'"
//@ diag: 32:22 error "initializer contains unexpanded parameter pack 'Ts'"
template<class... Ts>
constexpr bool e1 = requires(const Ts& t) { t < t; };
template<class... Ts>
constexpr bool e2 = requires { typename Ts::type; };
template<class... Ts>
void e3() { static_assert(requires(Ts t) { t; }); }
template<class... Ts>
constexpr bool e4 = (requires(int x) { x; } && ...);
template<class T, class... Ts>
constexpr bool e5 = requires(T t) { (t + ... + Ts()); };
template<class... Ts>
constexpr bool e6 = requires(const Ts&... t, Ts u) { u; };
template<class... Ts>
void e7() { static_assert(sizeof(Ts) > 0); }
template<class... Ts> struct S { static_assert(requires(Ts t) { t; }); };
template<class... Ts>
void e8() { bool b = requires(Ts t) { t; }; }
