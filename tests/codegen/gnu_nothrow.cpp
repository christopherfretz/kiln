// GNU `nothrow` (WP-186; clang `EST_NoThrow`): a function type attribute
// making the function non-throwing, so declarations, definitions and
// calls are `nounwind` and a call needs no landing pad, on a function,
// a member function and a pointer to function, written after the
// declarator or as `[[gnu::nothrow]]` before it.
void a() __attribute__((nothrow));
[[gnu::nothrow]] void b();
struct S { void m() __attribute__((nothrow)); ~S(); };
void (*fp)() __attribute__((nothrow));
void d() __attribute__((nothrow)) {}
void use() { S s; a(); b(); s.m(); fp(); d(); }
