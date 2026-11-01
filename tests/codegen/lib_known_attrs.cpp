// C library functions clang recognizes as library builtins get the
// implicit NoThrow / Const / Pure attributes of their Builtins.td row
// (nounwind; willreturn memory(none|read)) on their declarations and
// call sites, and so does an enclosing definition (WP-171). `printf`
// (a library builtin without NoThrow), a C++-linkage `tan` and a
// mismatched `strncmp` stay unwinding; written const/pure attributes
// give the same IR attributes, on definitions too.
typedef struct _IO_FILE FILE;
extern "C" {
int fprintf(FILE *, const char *, ...);
int snprintf(char *, unsigned long, const char *, ...);
int vsnprintf(char *, unsigned long, const char *, __builtin_va_list);
unsigned long strlen(const char *);
int strcmp(const char *, const char *);
double sin(double);
int isdigit(int);
int tolower(int);
unsigned long wcslen(const wchar_t *);
int printf(const char *, ...);
}
namespace n { extern "C" double cos(double); }
extern "C" unsigned long strlen(const char *);
extern "C++" double tan(double);
extern "C" int strncmp(const char *, const char *, int);

int format(char *buf, const char *f, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, f);
  int r = vsnprintf(buf, 64, f, ap);
  __builtin_va_end(ap);
  return r;
}

unsigned long use(FILE *f, char *buf, const wchar_t *w) {
  snprintf(buf, 8, "%d", 1);
  fprintf(f, "x");
  return strlen(buf) + strcmp(buf, "a") + isdigit(buf[0]) + tolower('A') + wcslen(w) +
         (unsigned long)(sin(1.0) + n::cos(2.0));
}

unsigned long unwinding(const char *s) {
  printf("y");
  return strncmp(s, "", 1) + (unsigned long)tan(3.0);
}

__attribute__((const)) int sq(int);
[[gnu::pure]] int rd(const int *);
__attribute__((const)) int twice(int x) { return x * 2; }
struct T { ~T(); };
int written(int *p) { T t; return sq(1) + rd(p) + twice(3); }
