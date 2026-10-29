// [temp.deduce.type]/8: cv-qualifiers and references deduced in partial
// specializations (the <type_traits> remove_* family).
template <class T> struct remove_const { using type = T; };
template <class T> struct remove_const<const T> { using type = T; };
template <class T> struct remove_reference { using type = T; };
template <class T> struct remove_reference<T &> { using type = T; };
template <class T> struct remove_reference<T &&> { using type = T; };
template <class T, class U> struct is_same { static constexpr bool value = false; };
template <class T> struct is_same<T, T> { static constexpr bool value = true; };
static_assert(is_same<remove_const<const int>::type, int>::value);
static_assert(is_same<remove_const<const volatile int>::type, volatile int>::value);
static_assert(is_same<remove_const<const int *>::type, const int *>::value);
static_assert(is_same<remove_reference<int &>::type, int>::value);
static_assert(is_same<remove_reference<const int &&>::type, const int>::value);
static_assert(is_same<remove_reference<int>::type, int>::value);
