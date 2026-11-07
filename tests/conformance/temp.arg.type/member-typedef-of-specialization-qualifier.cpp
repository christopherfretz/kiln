// [temp.arg.general]/2, [basic.lookup.qual]: in a template argument,
// `X<args>::m::name` where `m` is a member typedef of the class template
// `X` naming another class template specialization (`using jp =
// ptr<S>;`), and `name` is a member type of that class, is a type-id,
// also when the outer qualifier is a typedef of `X<args>` and when
// neither specialization has been instantiated yet (a JSON-pointer
// `string_t` shape, `is_same<A, oj::json_pointer::string_t>`). A static
// data member reached the same way stays an expression.
//@ expect: run
template <class R> struct ptr {
  using string_t = R;
  static constexpr int width = sizeof(R);
};
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
template <class K, class V> struct omap {};
template <template <class, class> class M = omap, class S = long> struct basic {
  using json_pointer = ::ptr<S>;
  using alias = json_pointer;
};
using oj = basic<omap>;
static_assert(same<long, oj::json_pointer::string_t>::value);
static_assert(same<long, basic<omap, long>::json_pointer::string_t>::value);
static_assert(same<char, basic<omap, char>::alias::string_t>::value);
template <int N> struct I { static constexpr int v = N; };
static_assert(I<basic<omap, short>::json_pointer::width>::v == sizeof(short));
template <class T> struct W { using type = T; };
W<basic<omap, int>::json_pointer::string_t>::type g = 3;
int main() { return g == 3 ? 0 : 1; }
