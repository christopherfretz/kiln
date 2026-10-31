//@ expect: run
//@ strict-diags
//@ stdout: 3 1 1 1 1
// [temp.names]/4, [expr.post.general]: the `<` of a named cast opens its
// own type list, so its `>` does not close an enclosing template argument
// list (libc++ <span> __integral_constant_like).
#include <stdio.h>

template <class T, T v> struct ic {
  static constexpr T value = v;
  constexpr operator T() const { return v; }
};
template <auto V> struct C { static constexpr auto value = V; };
template <bool B> struct bc { static constexpr bool value = B; };

template <class T> using X = C<static_cast<decltype(T::value)>(T())>;
template <class T>
concept like = bc<static_cast<decltype(T::value)>(T()) == T::value>::value;
template <class T>
constexpr bool rc =
    C<sizeof(reinterpret_cast<const char *>(T::value))>::value == sizeof(void *);
template <class T>
constexpr bool cc = C<const_cast<int *>(T::cvalue)>::value == nullptr;
struct Base { virtual ~Base() = default; };
struct Der : Base {};
template <class T>
constexpr bool dc = C<dynamic_cast<const Base *>(T::value)>::value == nullptr;
template <class T>
constexpr bool sc = C<static_cast<long>(T::value) + 1>::value == 4;

struct Z {
  static constexpr int *value = nullptr;
  static constexpr int *const cvalue = nullptr;
};
struct D { static constexpr const Der *value = nullptr; };

static_assert(X<ic<int, 3>>::value == 3);
static_assert(like<ic<int, 3>>);
static_assert(rc<Z> && cc<Z> && dc<D> && sc<ic<int, 3>>);

int main() {
  constexpr bool l = like<ic<long, 2>>;
  printf("%d %d %d %d %d\n", X<ic<int, 3>>::value, (int)l,
         (int)rc<Z>, (int)dc<D>, (int)sc<ic<int, 3>>);
}
