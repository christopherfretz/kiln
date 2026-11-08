// [temp.spec.partial.general]/9, [temp.arg.template]/1, [temp.variadic]/5:
// a partial specialization's argument list may expand a template template
// parameter pack (`S<T, void_t<Checks<T>...>, Checks...>`) against the
// primary's template template parameter pack; the expansion is checked as
// its pattern, a template. kiln rejected it with "template argument for
// template template parameter must be a class template or type alias
// template" (the N4502 detection idiom of a task-parallel runtime's
// `supports` helper, WP-803).
//@ expect: run
//@ stdout: 1 0 1 1 0
extern "C" int printf(const char*, ...);

template <typename...> struct void_impl { using type = void; };
template <typename... Args> using void_t = typename void_impl<Args...>::type;

template <typename T, typename, template <typename> class... Checks>
struct supports_impl { static constexpr int value = 0; };
template <typename T, template <typename> class... Checks>
struct supports_impl<T, void_t<Checks<T>...>, Checks...> { static constexpr int value = 1; };
template <typename T, template <typename> class... Checks>
constexpr int supports = supports_impl<T, void, Checks...>::value;

template <typename T> using has_size = decltype(T{}.size());
template <typename T> using has_begin = decltype(T{}.begin());
struct Box { int size() const { return 1; } int begin() const { return 0; } };
struct Plain {};

// A pack expansion after a non-pack argument of the primary.
template <typename T, typename U, template <typename> class... Cs> struct Q {
  static constexpr int value = 0;
};
template <typename T, template <typename> class... Cs> struct Q<T, int, Cs...> {
  static constexpr int value = sizeof...(Cs);
};
template <typename> struct A {};

int main() {
  printf("%d %d %d %d %d\n", supports<Box, has_size, has_begin>, supports<Plain, has_size>,
         supports<Box>, Q<char, int, A>::value, Q<char, long, A>::value);
}
