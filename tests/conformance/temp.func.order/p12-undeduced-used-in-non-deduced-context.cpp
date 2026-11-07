//@ expect: run
//@ stdout: 2 2 1 3 4
// WP-726: [temp.deduct.partial]/12: a template parameter left without a
// value after deduction fails it when the compared parameter types name
// it, a non-deduced context included; one they do not name may stay
// without a value. fmt's `to_pointer<Char>(it, n)` (folly json).
#include <cstddef>
#include <cstdio>
#include <type_traits>

struct appender {};
template <class T> struct bii {};
template <class T>
using buffer_appender =
    std::conditional_t<std::is_same<T, char>::value, appender, bii<T>>;

template <typename T, typename OutputIt>
constexpr int to_pointer(OutputIt, std::size_t) { return 1; }
template <typename T>
int to_pointer(buffer_appender<T>, std::size_t) { return 2; }

// `R` is named only by the return type: not compared, so it may stay
// without a value and #4 is more specialized than #3.
template <class R, class U> int g(U) { return 3; }
template <class R, class U> int g(U*) { return 4; }

int main() {
  appender a;
  bii<int> b;
  double d = 0;
  int i = 0;
  std::printf("%d %d %d %d %d\n", to_pointer<char>(a, 1), to_pointer<int>(b, 1),
              to_pointer<char>(d, 1), g<long>(i), g<long>(&i));
}
