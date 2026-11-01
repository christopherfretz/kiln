// [class.union.anon], [class.mem.general]: unnamed member classes of a class template are
// instantiated with it (the libc++ basic_string __short/__long layouts); sizes and offsets match clang
//@ expect: run
//@ strict-diags
struct N { struct { int a; }; char b; };
static_assert(__builtin_offsetof(N, b) == 4);
template <class C> struct str {
  typedef unsigned long size_type;
  typedef C* pointer;
  struct __long {
    struct __attribute__((packed)) {
      size_type __is_long_ : 1;
      size_type __cap_ : sizeof(size_type) * 8 - 1;
    };
    size_type __size_;
    pointer __data_;
  };
  enum { __min_cap = (sizeof(__long) - 1) / sizeof(C) > 2 ? (sizeof(__long) - 1) / sizeof(C) : 2 };
  struct __short {
    struct __attribute__((packed)) {
      unsigned char __is_long_ : 1;
      unsigned char __size_ : 7;
    };
    char __padding_[sizeof(C) - 1];
    C __data_[__min_cap];
  };
  static_assert(sizeof(__short) == (sizeof(C) * (__min_cap + 1)), "__short has an unexpected size.");
  // alternate layout
  struct __long2 {
    pointer __data_;
    size_type __size_;
    size_type __cap_ : sizeof(size_type) * 8 - 1;
    size_type __is_long_ : 1;
  };
  struct __short2 {
    C __data_[__min_cap];
    unsigned char __padding_[sizeof(C) - 1];
    unsigned char __size_ : 7;
    unsigned char __is_long_ : 1;
  };
  union __rep { __short __s; __long __l; };
  union { int u1; float u2; };
  struct { int a, b; } named;
  __rep r;
  bool is_long() const { return r.__s.__is_long_; }
  size_type cap() const { return r.__l.__cap_; }
  void set(size_type n) { r.__l.__cap_ = n; r.__l.__is_long_ = 1; u1 = 3; named.b = 4; }
  int get() const { return u1 + named.a; }
};
template struct str<char>;
template struct str<wchar_t>;
template struct str<char16_t>;
#define S(C) \
static_assert(sizeof(str<C>::__long) == 24); \
static_assert(sizeof(str<C>::__short) == 24); \
static_assert(__builtin_offsetof(str<C>::__short, __data_) == sizeof(C)); \
static_assert(__builtin_offsetof(str<C>::__long, __size_) == 8); \
static_assert(sizeof(str<C>::__long2) == 24); \
static_assert(sizeof(str<C>::__short2) == 24); \
static_assert(sizeof(str<C>) == 40); \
static_assert(__builtin_offsetof(str<C>, r) == 16);
S(char) S(wchar_t) S(char16_t) S(char32_t)
int main() {
  str<char> s{};
  s.set(100);
  if (!s.is_long() || s.cap() != 100) return 1;
  if (s.get() != 3) return 2;
  return 0;
}
