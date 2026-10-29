// [cstring.syn], [cwchar.syn]: the memory and string builtins. memcpy and
// memmove are intrinsics with the alignment the pointer arguments are known
// to have; the rest call the library function (strlen of a literal folds).
using size_t = decltype(sizeof 0);
int a(const void* p, const void* q, size_t n) { return __builtin_memcmp(p, q, n); }
void* b(void* p, const void* q, size_t n) { return __builtin_memcpy(p, q, n); }
void* c(void* p, const void* q, size_t n) { return __builtin_memmove(p, q, n); }
const void* d(const void* p, int ch, size_t n) { return __builtin_memchr(p, ch, n); }
const char* e(const char* p, int ch, size_t n) { return __builtin_char_memchr(p, ch, n); }
size_t f(const char* p) { return __builtin_strlen(p); }
size_t g(const wchar_t* p) { return __builtin_wcslen(p); }
const wchar_t* h(const wchar_t* p, wchar_t c, size_t n) { return __builtin_wmemchr(p, c, n); }
int i(const wchar_t* p, const wchar_t* q, size_t n) { return __builtin_wmemcmp(p, q, n); }
struct S { int x[4]; double d; };
struct I;
S gs;
char buf[32];
void j(S* p, S* q) { __builtin_memcpy(p, q, sizeof(S)); __builtin_memmove(p, q, 8); }
size_t k(const char* s) { return __builtin_strlen(s) + __builtin_strlen("ab\0c") + __builtin_strlen("hello"); }
void l(S* p, const void* q) { __builtin_memcpy((char*)p, q, 4); __builtin_memcpy(&gs, buf, 8); }
void m(I* p, I* q) { __builtin_memmove(p, q, 1); }
void n(int* p) { __builtin_memcpy((void*)p, (const void*)&gs, 4); __builtin_memcpy(p + 1, gs.x, 4); }
