// The implicit format(scanf, 1, 2) attribute of the library scanf family
// (scanf, sscanf, fscanf; vscanf / vsscanf with no data arguments) and the
// -Wformat checks of a literal scanf format (GNU [dcl.attr] extension;
// clang Sema::AddKnownFunctionAttributes, CheckScanfHandler): each
// conversion's argument must point to the type it specifies (signedness
// ignored; a const pointee never matches, a volatile one does; an unscoped
// enumeration as its underlying type; `%p` wants `void **`), `*` suppresses
// the assignment and consumes no argument, an invalid specifier consumes
// none, more conversions than arguments stop the walk, and unused
// arguments are reported. Verified against clang 19 and 22. WP-1073.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 56:15 warning "format specifies type 'int *' but the argument has type 'float *'"
//@ diag: 58:15 warning "format specifies type 'float *' but the argument has type 'double *'"
//@ diag: 59:24 warning "format specifies type 'double *' but the argument has type 'float *'"
//@ diag: 61:16 warning "format specifies type 'long double *' but the argument has type 'double *'"
//@ diag: 63:15 warning "format specifies type 'int *' but the argument has type 'long *'"
//@ diag: 64:16 warning "format specifies type 'long *' but the argument has type 'int *'"
//@ diag: 67:16 warning "format specifies type 'short *' but the argument has type 'int *'"
//@ diag: 70:15 warning "format specifies type 'char *' but the argument has type 'int *'"
//@ diag: 71:18 warning "format specifies type 'char *' but the argument has type 'int *'"
//@ diag: 73:15 warning "format specifies type 'void **' but the argument has type 'int *'"
//@ diag: 74:15 warning "format specifies type 'void **' but the argument has type 'void *const *'"
//@ diag: 75:15 warning "format specifies type 'int *' but the argument has type 'const int *'"
//@ diag: 77:22 warning "format specifies type 'int *' but the argument has type 'F *'"
//@ diag: 78:15 warning "format specifies type 'int *' but the argument has type 'bool *'"
//@ diag: 79:15 warning "format specifies type 'int *' but the argument has type 'int'"
//@ diag: 81:14 warning "more '%' conversions than data arguments"
//@ diag: 82:19 warning "data argument not used by format string"
//@ diag: 83:11 warning "invalid conversion specifier 'k'"
//@ diag: 84:14 warning "invalid conversion specifier 'y'"
//@ diag: 84:22 warning "data argument not used by format string"
//@ diag: 85:10 warning "incomplete format specifier"
//@ diag: 86:10 warning "incomplete format specifier"
//@ diag: 88:21 warning "format specifies type 'int *' but the argument has type 'float *'"
//@ diag: 89:20 warning "format specifies type 'int *' but the argument has type 'float *'"
//@ diag: 90:15 warning "format specifies type 'int *' but the argument has type 'void *'"
typedef __SIZE_TYPE__ size_t;
typedef __PTRDIFF_TYPE__ ptrdiff_t;
typedef __builtin_va_list va_list;
struct FILE;
extern "C" {
int scanf(const char *, ...);
int sscanf(const char *, const char *, ...);
int fscanf(FILE *, const char *, ...);
int vscanf(const char *, va_list);
int vsscanf(const char *, const char *, va_list);
}
enum E { e0 };
enum class F { f0 };
void f(int *ip, unsigned *up, long *lp, float *fp, double *dp, long double *ldp,
       char *cp, short *sp, signed char *scp, unsigned char *ucp, void **vpp,
       size_t *zp, const int *cip, volatile int *vip, long long *llp, int iv,
       ptrdiff_t *tp, E *ep, F *fep, bool *bp, void *const *cvpp, FILE *fi) {
  scanf("%d %i %u %x %o %X %n", ip, ip, ip, ip, up, ip, ip);
  scanf("%d", fp);
  scanf("%f %e %g %a %E %G %A %F", fp, fp, fp, fp, fp, fp, fp, fp);
  scanf("%f", dp);
  scanf("%lf %le", dp, fp);
  scanf("%Lf", ldp);
  scanf("%Lf", dp);
  scanf("%ld %lld %qd %Ld %jd", lp, llp, llp, llp, lp);
  scanf("%d", lp);
  scanf("%ld", ip);
  scanf("%hd %hu %hhd %hhu %hhn", sp, sp, scp, ucp, scp);
  scanf("%hhd %hhu", cp, cp);
  scanf("%hd", ip);
  scanf("%zu %zd %td", zp, zp, tp);
  scanf("%s %c %10s %[abc] %[^x] %c", cp, cp, cp, cp, cp, ucp);
  scanf("%s", ip);
  scanf("%[^x]", ip);
  scanf("%p", vpp);
  scanf("%p", ip);
  scanf("%p", cvpp);
  scanf("%d", cip);
  scanf("%d", vip);
  scanf("%d %d", ep, fep);
  scanf("%d", bp);
  scanf("%d", iv);
  scanf("%*d %d %%", ip);
  scanf("%d %d", ip);
  scanf("%d", ip, ip);
  scanf("%k", ip);
  scanf("%d %y", ip, ip);
  scanf("%", ip);
  scanf("%5");
  scanf("%2$d %1$d", ip, ip);
  sscanf("a", "%d", fp);
  fscanf(fi, "%d", fp);
  scanf("%d", (void *)ip);
}
void g(va_list ap) {
  vscanf("%d", ap);
  vsscanf("1", "%d %d", ap);
}
