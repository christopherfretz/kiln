// clang Builtin::Context (WP-1073): a library function declared with a
// signature other than the library's is not the builtin, so it gets none of
// the builtin's implicit attributes (no `allocsize`, `nounwind` only from
// `noexcept`, no `format(scanf)` check).
extern "C" char* malloc(unsigned long);
extern "C" void* calloc(int, unsigned long);
extern "C" void* realloc(void*, unsigned long) noexcept;
void* f(unsigned long n) {
  return malloc(n) + (long)calloc(1, n) + (long)realloc(0, n);
}
extern "C" int sscanf(const char*, const char*, ...);
void g(char* p) { sscanf("", "%s", p); }
