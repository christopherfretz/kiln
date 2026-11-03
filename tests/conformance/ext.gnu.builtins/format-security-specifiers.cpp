// -Wformat family on calls through a printf-format function (WP-186; clang
// checkFormatString/checkFormatStringExpr): the C library printf family
// carries an implicit format(printf) attribute; a non-literal format with
// no data arguments warns -Wformat-security (null pointer constants,
// __func__ and const-qualified variables with an initializer are not
// checked); an invalid conversion specifier warns
// -Wformat-invalid-specifier and covers the next argument, an incomplete
// one stops the check; across the arms of a non-constant conditional an
// argument is unused only if every arm leaves it uncovered, and a
// constant condition checks the taken arm only. Verified against clang 22
// and 19.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 54:10 warning "format string is not a string literal (potentially insecure)"
//@ diag: 54:10 note "treat the string as an argument to avoid this"
//@ diag: 55:14 warning "format string is not a string literal (potentially insecure)"
//@ diag: 55:14 note "treat the string as an argument to avoid this"
//@ diag: 56:16 warning "format string is not a string literal (potentially insecure)"
//@ diag: 56:16 note "treat the string as an argument to avoid this"
//@ diag: 57:29 warning "format string is not a string literal (potentially insecure)"
//@ diag: 57:29 note "treat the string as an argument to avoid this"
//@ diag: 58:11 warning "format string is not a string literal (potentially insecure)"
//@ diag: 58:11 note "treat the string as an argument to avoid this"
//@ diag: 59:10 warning "format string is not a string literal (potentially insecure)"
//@ diag: 59:10 note "treat the string as an argument to avoid this"
//@ diag: 65:12 warning "invalid conversion specifier 'y'"
//@ diag: 65:24 warning "data argument not used by format string"
//@ diag: 66:12 warning "invalid conversion specifier 'y'"
//@ diag: 67:12 warning "invalid conversion specifier 'y'"
//@ diag: 68:14 warning "incomplete format specifier"
//@ diag: 69:14 warning "incomplete format specifier"
//@ diag: 70:12 warning "invalid conversion specifier '\\x0a'"
//@ diag: 71:12 warning "invalid conversion specifier '\"'"
//@ diag: 73:14 warning "invalid conversion specifier 'k'"
//@ diag: 73:26 warning "data argument not used by format string"
//@ diag: 78:33 warning "data argument not used by format string"
//@ diag: 79:34 warning "data argument not used by format string"
//@ diag: 81:32 warning "more '%' conversions than data arguments"
//@ diag: 82:19 warning "more '%' conversions than data arguments"
typedef __SIZE_TYPE__ size_t;
struct FILE;
extern "C" {
int printf(const char *, ...);
int fprintf(FILE *, const char *, ...);
int sprintf(char *, const char *, ...);
int snprintf(char *, size_t, const char *, ...);
}
void logf(int level, const char *fmt, ...) __attribute__((format(printf, 2, 3)));

const char *msg();
char buf[32];

void security(const char *s, FILE *f, bool b) {
  printf(s);
  fprintf(f, s);
  sprintf(buf, s);
  snprintf(buf, sizeof buf, msg());
  logf(1, s);
  printf(b ? "a" : s);
  printf(s, 1);
  printf(__func__);
}

void specifiers(int i, double d) {
  printf("%y %d\n", i, i);
  printf("%y\n", i);
  printf("%y %d\n", i);
  printf("%d %", i);
  printf("%d %", i, i);
  printf("%\n");
  printf("%\"%d", i);
  printf("%1$d", i);
  logf(0, "%5k|%d\n", i, d);
}

void arms(int i, bool b) {
  printf(b ? "%d\n" : "%d %d\n", i, i);
  printf(b ? "%d\n" : "x\n", i, i);
  printf(b ? "%d\n" : "%d\n", i, i);
  printf(true ? "%d\n" : "%s\n", i);
  printf(false ? "%d\n" : "%d %d\n", i);
  printf(b ? "%d %d\n" : "%d\n", i);
}
