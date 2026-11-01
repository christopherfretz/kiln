// [cstring.syn], [cwchar.syn]: the C string builtins call the library
// function (the wcs* declarations are pure: memory(read)); an integer result
// with constant operands folds.
using size_t = decltype(sizeof 0);
int a(const char* p, const char* q) { return __builtin_strcmp(p, q); }
int b(const char* p, const char* q, size_t n) { return __builtin_strncmp(p, q, n); }
int c(const wchar_t* p, const wchar_t* q) { return __builtin_wcscmp(p, q); }
int d(const wchar_t* p, const wchar_t* q, size_t n) { return __builtin_wcsncmp(p, q, n); }
const char* e(const char* p, int ch) { return __builtin_strchr(p, ch); }
const wchar_t* f(const wchar_t* p, wchar_t ch) { return __builtin_wcschr(p, ch); }
const char* g(const char* p, int ch) { return __builtin_strrchr(p, ch); }
const char* h(const char* p, const char* q) { return __builtin_strpbrk(p, q); }
const char* i(const char* p, const char* q) { return __builtin_strstr(p, q); }
int j() { return __builtin_strcmp("ab", "ac") + __builtin_strncmp("abc", "abd", 2) + __builtin_wcscmp(L"x", L"x"); }
