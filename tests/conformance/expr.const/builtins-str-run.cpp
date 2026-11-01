// [cstring.syn], [cwchar.syn]: at run time the string builtins call the C
// library (operands read from volatiles, so nothing folds) and agree with
// the constant-evaluated results; strrchr, strstr and strpbrk are
// library calls only.
//@ expect: run
//@ stdout: 0 -1 1 0 -1
//@ stdout: 0 -1 0
//@ stdout: 2 -1 2 4
//@ stdout: 2 -1
//@ stdout: 3 1 2 -1
//@ stdout: 1 0
extern "C" int printf(const char *, ...);
typedef decltype(sizeof 0) size_t;
static int sgn(int v) { return (v > 0) - (v < 0); }
template <class C> static long at(const C *s, const C *p) { return p ? p - s : -1; }
const char *volatile va = "abc";
const char *volatile vb = "abd";
const char *volatile vc = "ab";
const wchar_t *volatile wa = L"wide";
const wchar_t *volatile wb = L"wida";
volatile int vz = 'z';
volatile size_t v3 = 3;
constexpr int folded = __builtin_strcmp("kiln", "kilm");
int main() {
  const char *a = va, *b = vb, *c = vc;
  printf("%d %d %d %d %d\n", sgn(__builtin_strcmp(a, a)), sgn(__builtin_strcmp(a, b)),
         sgn(__builtin_strcmp(a, c)), sgn(__builtin_strncmp(a, b, 2)),
         sgn(__builtin_strncmp(a, b, v3)));
  const wchar_t *w = wa, *x = wb;
  printf("%d %d %d\n", sgn(__builtin_wcscmp(w, w)), sgn(__builtin_wcscmp(x, w)),
         sgn(__builtin_wcsncmp(x, w, v3)));
  printf("%ld %ld %ld %ld\n", at(a, __builtin_strchr(a, 'c')), at(a, __builtin_strchr(a, vz)),
         at(c, __builtin_strchr(c, 0)),
         at(w, __builtin_wcschr(w, 0)));
  printf("%ld %ld\n", at(w, __builtin_wcschr(w, L'd')), at(w, __builtin_wcschr(w, vz)));
  const char *s = "abcabc";
  printf("%ld %ld %ld %ld\n", at(s, __builtin_strrchr(s, 'a')), at(s, __builtin_strstr(s, "bc")),
         at(s, __builtin_strpbrk(s, "xc")), at(s, __builtin_strstr(s, "x")));
  printf("%d %d\n", folded, __builtin_strcmp("same", "same"));
}
