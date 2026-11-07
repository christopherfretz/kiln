//@ expect: run
//@ std: c++20
//@ stdout: 5 1 1 0 7 1 9
// [temp.expl.spec]/3 (CWG 727): a member variable template is explicitly
// specialized in its class, without `static`; in a class template the
// specialization is instantiated with the class, its initializer parsed
// in place (not as a default member initializer).
extern "C" int printf(const char *, ...);

template <class B> struct H {
  template <typename> static inline constexpr bool sup_v = false;
  template <> inline constexpr bool sup_v<int> = true;
  template <auto Tag> static inline constexpr bool tag_v = false;
  template <> inline constexpr bool tag_v<3> = true;
  template <class T> static inline constexpr int size_v = 0;
  template <> inline constexpr int size_v<B> = sizeof(B) + 3;
  static int f() { return sup_v<int> * 4 + sup_v<char> * 2 + tag_v<3>; }
};

struct P {
  template <typename> static inline constexpr bool sup_v = false;
  template <> inline constexpr bool sup_v<int> = true;
};

static_assert(P::sup_v<int> && !P::sup_v<long>);
static_assert(H<int>::size_v<int> == sizeof(int) + 3);

const bool *addr() { return &P::sup_v<int>; }
const int *addr2() { return &H<int>::size_v<int>; }

int main() {
  printf("%d %d %d %d %d %d %d\n", H<long>::f(), (int)P::sup_v<int>, (int)*addr(),
         (int)H<char>::sup_v<long>, *addr2(), (int)H<long>::tag_v<3>,
         H<char[6]>::size_v<char[6]>);
}
