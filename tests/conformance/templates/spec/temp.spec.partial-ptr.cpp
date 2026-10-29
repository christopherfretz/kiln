// [temp.spec.partial]/1, [temp.spec.partial.match]/2: a class template
// partial specialization is used for the arguments it matches.
template <class T> struct is_pointer { static constexpr bool value = false; };
template <class T> struct is_pointer<T *> { static constexpr bool value = true; };
template <class T> struct pointee { using type = void; };
template <class T> struct pointee<T *> { using type = T; };
static_assert(!is_pointer<int>::value);
static_assert(is_pointer<int *>::value);
static_assert(is_pointer<void (*)()>::value);
static_assert(sizeof(pointee<long *>::type) == 8);
static_assert(sizeof(pointee<pointee<char **>::type>::type) == 1);
