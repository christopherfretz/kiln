// [temp.over.link]/6-7, [temp.constr.decl]: a trailing requires-clause
// on a redeclaration must be equivalent; spelled with other parameter
// names it is.
template <class T> struct is_big { static constexpr bool value = sizeof(T) > 1; };
template <class T> int k(T) requires (is_big<T>::value && sizeof(T) <= 8);
template <class U> int k(U) requires (is_big<U>::value && sizeof(U) <= 8) { return 1; }
int r = k(1);
