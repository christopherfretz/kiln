// [except.spec]/5, [temp.over.link]/5: a dependent noexcept-specifier on
// a redeclaration is compatible when its expression is equivalent.
template <class T> struct big { static const bool value = sizeof(T) > 1; };
template <class T> void g(T) noexcept(big<T>::value && sizeof(T) < 64);
template <class U> void g(U) noexcept(big<U>::value && sizeof(U) < 64) {}
static_assert(noexcept(g(1)));
static_assert(!noexcept(g('c')));
