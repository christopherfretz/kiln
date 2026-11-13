// [temp.arg.nontype]/6 (WP-1362): the valid neighbours of a string literal
// inside a class-type argument keep their mangling: a `fixed_string`
// holding a `char[N]` copy of the literal (`Xtl…E` of its elements), a
// pointer to a named `static constexpr char[]` object, and a null pointer.
namespace wp1362 {
template <unsigned N> struct FS {
  constexpr FS(const char (&s)[N]) {
    for (unsigned i = 0; i < N; ++i) v[i] = s[i];
  }
  char v[N]{};
};
struct P { const char *p; };
struct Q { const char *a[2]; };
static constexpr char named[] = "nm";
constexpr char ext[] = "ex";
template <FS s> void f() {}
template <P p> void g() {}
template <auto V> void h() {}
void use() {
  f<"abc">();
  f<"">();
  f<FS{"a\0b"}>();
  g<P{named}>();
  g<P{named + 1}>();
  g<P{ext}>();
  g<P{nullptr}>();
  h<Q{{ext, nullptr}}>();
  h<FS{"xy"}>();
}
} // namespace wp1362
