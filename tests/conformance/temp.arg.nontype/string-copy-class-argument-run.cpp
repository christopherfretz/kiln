// [temp.arg.nontype]/6 (WP-1362): the subobject rule rejects a pointer to
// a string literal inside a class-type argument, but not its valid
// neighbours: a `fixed_string` holding a `char[N]` copy of the literal,
// a pointer to (into) a named `static constexpr char[]` object, and a
// null pointer. Each specialization is distinct, defined and callable.
//@ expect: run
//@ strict-diags
//@ stdout: abc 3|nm m ex null|xy 2
extern "C" int printf(const char*, ...);
template <unsigned N> struct FS {
  constexpr FS(const char (&s)[N]) {
    for (unsigned i = 0; i < N; ++i) v[i] = s[i];
  }
  char v[N]{};
};
struct P { const char* p; };
static constexpr char named[] = "nm";
constexpr char ext[] = "ex";
template <FS s> const char* str() { return s.v; }
template <FS s> unsigned len() { return sizeof(s.v) - 1; }
template <P p> const char* ptr() { return p.p ? p.p : "null"; }
template <auto V> const char* any() { return V.v; }
int main() {
  printf("%s %u|", str<"abc">(), len<"abc">());
  printf("%s %s %s %s|", ptr<P{named}>(), ptr<P{named + 1}>(), ptr<P{ext}>(),
         ptr<P{nullptr}>());
  printf("%s %u\n", any<FS{"xy"}>(), len<"xy">());
}
