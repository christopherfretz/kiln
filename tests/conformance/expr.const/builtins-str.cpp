// [expr.const], [cstring.syn], [cwchar.syn], [type.info]: the string
// comparison and search builtins clang 19 evaluates (strcmp, strncmp,
// wcscmp, wcsncmp, strchr, wcschr), which libc++'s type_info and
// <string.h>/<wchar.h> use, are constant-evaluable as clang evaluates them:
// equal at the first null, unsigned narrow comparison, wchar_t by its own signedness, a
// strchr value the element type cannot hold is never found, and each is one
// native builtin step, not a loop in the evaluator.
//@ expect: compile-ok
//@ strict-diags
//@ oven: steps<=8400 allocs<=0
typedef decltype(sizeof 0) size_t;

static_assert(__builtin_strcmp("", "") == 0);
static_assert(__builtin_strcmp("abc", "abc") == 0);
static_assert(__builtin_strcmp("abc", "abd") == -1);
static_assert(__builtin_strcmp("abd", "abc") == 1);
static_assert(__builtin_strcmp("ab", "abc") == -1);
static_assert(__builtin_strcmp("abc", "ab") == 1);
static_assert(__builtin_strcmp("a\0x", "a\0y") == 0);
static_assert(__builtin_strcmp("\xff", "a") == 1);
static_assert(__builtin_strncmp("abcx", "abcy", 3) == 0);
static_assert(__builtin_strncmp("abcx", "abcy", 4) == -1);
static_assert(__builtin_strncmp("ab", "ab", 100) == 0);
static_assert(__builtin_strncmp("x", "y", 0) == 0);
static_assert(__builtin_strncmp(nullptr, nullptr, 0) == 0);
static_assert(__builtin_wcscmp(L"wide", L"wide") == 0);
static_assert(__builtin_wcscmp(L"wida", L"wide") == -1);
static_assert(__builtin_wcsncmp(L"wideA", L"wideB", 4) == 0);
static_assert(__builtin_wcsncmp(L"wideA", L"wideB", 5) == -1);

constexpr const char *hello = "hello";
static_assert(__builtin_strchr(hello, 'l') == hello + 2);
static_assert(__builtin_strchr(hello, 'o') == hello + 4);
static_assert(__builtin_strchr(hello, 0) == hello + 5);
static_assert(__builtin_strchr(hello, 'z') == nullptr);
static_assert(__builtin_strchr(hello, 'l' + 256) == nullptr);
static_assert(__builtin_strchr("a\0b", 'b') == nullptr);
constexpr const wchar_t *wide = L"wide";
static_assert(__builtin_wcschr(wide, L'd') == wide + 2);
static_assert(__builtin_wcschr(wide, 0) == wide + 4);
static_assert(__builtin_wcschr(wide, L'z') == nullptr);

// Arrays that are not string literals, and a long comparison that costs
// one evaluator step (the 8200 steps are the two constructor loops).
constexpr char a1[] = {'k', 'i', 'l', 'n', 0, 'x'};
constexpr char a2[] = {'k', 'i', 'l', 'n', 0, 'y'};
static_assert(__builtin_strcmp(a1, a2) == 0);
static_assert(__builtin_strncmp(a1, a2, 6) == 0);
static_assert(__builtin_strchr(a1, 'x') == nullptr);
struct Big {
  char s[4096];
  constexpr Big(char last) : s{} {
    for (int i = 0; i < 4094; ++i) s[i] = 'a';
    s[4094] = last;
  }
};
constexpr Big b1('a'), b2('b');
static_assert(__builtin_strcmp(b1.s, b2.s) == -1);

// libc++'s type_info name comparison (`__type_name_to_string`).
constexpr bool name_lt(const char *l, const char *r) { return __builtin_strcmp(l, r) < 0; }
static_assert(name_lt("i", "j") && !name_lt("PKc", "PKc"));
