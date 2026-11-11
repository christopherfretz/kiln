// C17 7.1.4: a first explicit declaration of a C library function whose
// type is not compatible with the library's. clang's redeclaration lookup
// creates the builtin declaration (`LazilyCreateBuiltin`) and
// `MergeFunctionDecl` warns against it (-Wincompatible-library-redeclaration),
// noting the builtin's type at the new declaration (WP-1312, WP-1302
// follow-up). A compatible one, a `static` one or a block-scope one is
// not warned about; `_Exit` without `noreturn` is compatible.
//@ strict-diags
//@ diag: 21:5 warning "incompatible redeclaration of library function 'printf'"
//@ diag: 21:5 note "'printf' is a builtin with type 'int (const char *, ...)'"
//@ diag: 22:5 warning "incompatible redeclaration of library function 'abs'"
//@ diag: 22:5 note "'abs' is a builtin with type 'int (int)'"
//@ diag: 23:5 warning "incompatible redeclaration of library function 'toupper'"
//@ diag: 23:5 note "'toupper' is a builtin with type 'int (int)'"
//@ diag: 28:6 warning "incompatible redeclaration of library function 'exit'"
//@ diag: 28:6 note "'exit' is a builtin with type 'void (int) __attribute__((noreturn))'"
//@ diag: 29:5 warning "incompatible redeclaration of library function 'free'"
//@ diag: 29:5 note "'free' is a builtin with type 'void (void *)'"
//@ diag: 30:6 warning "incompatible redeclaration of library function 'strtol'"
//@ diag: 30:6 note "'strtol' is a builtin with type 'long (const char *, char **, int)'"
int printf(int);
int abs(long);
int toupper(char);
double fabs(double);
int puts(const char *) __attribute__((noreturn));
static int strlen(const char *);
void f(void) { extern int atoi(int); }
void exit(long);
int free(void *);
long strtol(const char *, char **);
void _Exit(int);
