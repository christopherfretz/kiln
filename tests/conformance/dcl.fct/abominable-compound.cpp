// [dcl.fct]/6: a function type with a cv-qualifier-seq or ref-qualifier
// (an "abominable" type) may not be the referent of a pointer or
// reference, whether written or formed by substitution (a SFINAE error,
// which is how std::declval / is_invocable reject such a type).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:7 error "pointer to function type cannot have 'const' qualifier"
//@ diag: 16:7 error "reference to function type cannot have '&' qualifier"
//@ diag: 17:3 error "pointer to function type 'AF' (aka 'void (...) const') cannot have 'const' qualifier"
//@ diag: 18:3 error "reference to function type 'AR' (aka 'void () volatile &&') cannot have 'volatile &&' qualifier"
//@ diag: 21 error "pointer to function type 'void () const' cannot have 'const' qualifier"
//@ diag: 23:17 note "in instantiation of template class 'W<void () const>' requested here"
using AF = void(...) const;
using AR = void() volatile &&;
void (*p)() const;
void (&r)() & = *(void (*)())0;
AF* q;
AR& s = *(void (*)())0;
template <class T>
struct W {
  T* p; // line only: clang points at the `*`, kiln at the name
};
W<void() const> w;

template <class T> T&& declval();
template <class T, class = decltype(declval<T>())> constexpr bool ok(int) { return true; }
template <class T> constexpr bool ok(...) { return false; }
static_assert(!ok<AF>(0) && !ok<AR>(0) && ok<void()>(0));
struct C { AF f; };
void (C::*mp)(...) const = &C::f;
