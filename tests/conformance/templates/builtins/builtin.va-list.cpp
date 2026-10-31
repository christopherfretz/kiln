// [cstdarg.syn] + the target ABI: __builtin_va_list is a predeclared type
// naming the target's va_list (AAPCS64: struct std::__va_list; x86-64 SysV:
// __va_list_tag[1]). The record's tag is not visible to ordinary lookup.
//@ expect: compile-ok
//@ strict-diags
typedef __builtin_va_list va_list;
#if defined(__aarch64__)
static_assert(sizeof(va_list) == 32 && alignof(va_list) == 8);
#elif defined(__x86_64__)
static_assert(sizeof(va_list) == 24 && alignof(va_list) == 8);
#endif
static_assert(__is_trivially_copyable(__builtin_va_list));
void f(int n, ...);
struct S { va_list ap; };
va_list g;
template <class T> struct is_va { static constexpr bool value = false; };
template <> struct is_va<__builtin_va_list> { static constexpr bool value = true; };
static_assert(is_va<va_list>::value);
static_assert(!is_va<int>::value);
namespace std { struct x {}; }   // reopening std is unaffected
int __va_list_tag;               // the tags do not collide with user names
int __va_list;
int main() { return 0; }
