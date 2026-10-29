// [expr.const], [char.traits.specializations]: the memory and string
// builtins libc++'s char_traits uses are constant-evaluable.
//@ expect: compile-ok
//@ strict-diags
typedef decltype(sizeof 0) size_t;

static_assert(__builtin_strlen("") == 0);
static_assert(__builtin_strlen("hello") == 5);
static_assert(__builtin_strlen("ab\0cd") == 2);
static_assert(__builtin_wcslen(L"wide") == 4);
constexpr char arr[] = {'x', 'y', 0, 'z'};
static_assert(__builtin_strlen(arr) == 2);
static_assert(__builtin_strlen(arr + 3 - 3) == 2);

static_assert(__builtin_memcmp("abc", "abd", 3) < 0);
static_assert(__builtin_memcmp("abc", "abd", 2) == 0);
static_assert(__builtin_memcmp("b", "a", 1) > 0);
static_assert(__builtin_memcmp("abc", "xyz", 0) == 0);
static_assert(__builtin_wmemcmp(L"abc", L"abc", 3) == 0);
static_assert(__builtin_wmemcmp(L"abc", L"abz", 3) < 0);

constexpr const char *hello = "hello";
static_assert(__builtin_memchr(hello, 'l', 5) == (const void *)(hello + 2));
static_assert(__builtin_memchr(hello, 'l' + 256, 5) == (const void *)(hello + 2));
static_assert(__builtin_char_memchr(hello, 'l', 5) == hello + 2);
static_assert(__builtin_char_memchr(hello, 'z', 5) == nullptr);
static_assert(__builtin_char_memchr(hello, 'o', 4) == nullptr);
constexpr const wchar_t *wide = L"wide";
static_assert(__builtin_wmemchr(wide, L'd', 4) == wide + 2);
static_assert(__builtin_wmemchr(wide, L'q', 4) == nullptr);

// memcpy/memmove on trivially-copyable objects inside constant evaluation.
constexpr int copy_sum() {
  int a[4] = {1, 2, 3, 4};
  int b[4] = {};
  __builtin_memcpy(b, a, sizeof a);
  return b[0] + b[1] + b[2] + b[3];
}
static_assert(copy_sum() == 10);

constexpr int overlap() {
  int a[5] = {1, 2, 3, 4, 5};
  __builtin_memmove(a + 1, a, 3 * sizeof(int));
  return a[0] * 10000 + a[1] * 1000 + a[2] * 100 + a[3] * 10 + a[4];
}
static_assert(overlap() == 11235);

constexpr int overlap_down() {
  char s[] = "abcdef";
  __builtin_memmove(s, s + 2, 3);
  return s[0] == 'c' && s[1] == 'd' && s[2] == 'e' && s[3] == 'd';
}
static_assert(overlap_down());

constexpr size_t my_strlen(const char *s) { return __builtin_strlen(s); }
constexpr char buf[] = "constexpr";
static_assert(my_strlen(buf) == 9);

// char_traits-shaped helpers.
constexpr int compare(const char *a, const char *b, size_t n) { return __builtin_memcmp(a, b, n); }
static_assert(compare("kiln", "kilo", 4) < 0);
constexpr const char *find(const char *s, size_t n, char c) { return __builtin_char_memchr(s, c, n); }
static_assert(*find("oven", 4, 'e') == 'e');
