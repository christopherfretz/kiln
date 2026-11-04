// [stmt.if]/4, [char.traits.require]: a library-shaped `length`/`copy`
// that loops under `if consteval` and calls the builtin otherwise. At run
// time only the else branch runs (the volatile counter sees every call,
// however the compiler folds), in a constant expression only the loop
// (WP-588).
//@ expect: run
//@ stdout: 5 3 hello 3
using size_t = decltype(sizeof 0);
extern "C" int printf(const char *, ...);
volatile int rt_calls;

struct traits {
  static constexpr size_t length(const char *s) {
    if consteval {
      size_t n = 0;
      while (s[n])
        ++n;
      return n;
    } else {
      rt_calls = rt_calls + 1;
      return __builtin_strlen(s);
    }
  }
  static constexpr char *copy(char *d, const char *s, size_t n) {
    if consteval {
      for (size_t i = 0; i < n; ++i)
        d[i] = s[i];
    } else {
      rt_calls = rt_calls + 1;
      __builtin_memcpy(d, s, n);
    }
    return d;
  }
};

constexpr size_t cl = traits::length("abc");
static_assert(cl == 3);
constexpr bool copied() {
  char b[4] = {};
  traits::copy(b, "xyz", 4);
  return b[0] == 'x' && b[2] == 'z' && b[3] == 0;
}
static_assert(copied());

int main() {
  size_t n = traits::length("hello");
  char buf[8] = {};
  if (traits::length("abc") == 3)
    traits::copy(buf, "hello", 6);
  printf("%d %d %s %d\n", static_cast<int>(n), static_cast<int>(cl), buf, static_cast<int>(rt_calls));
}
