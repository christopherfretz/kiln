// [over.literal]: literal operator declarations (clang CheckLiteralOperatorDeclaration)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:15 error "invalid suffix on literal; C++11 requires a space between literal and identifier"
//@ diag: 30:5 warning "user-defined literal suffixes not starting with '_' are reserved; no literal will invoke this operator"
//@ diag: 31:5 warning "user-defined literal suffixes not starting with '_' are reserved"
//@ diag: 32:18 error "invalid literal operator parameter type 'int', did you mean 'unsigned long long'?"
//@ diag: 33:31 error "invalid literal operator parameter type 'int', did you mean 'unsigned long'?"
//@ diag: 34:5 error "non-template literal operator must have one or two parameters"
//@ diag: 35:39 error "literal operator cannot have a default argument"
//@ diag: 36:16 error "literal operator 'operator\"\"_g' must be in a namespace or global scope"
//@ diag: 37:16 error "literal operator must have C++ linkage"
//@ diag: 37:1 note "extern \"C\" language linkage specification begins here"
//@ diag: 39:1 error "template parameter list for literal operator must be either 'char...' or 'typename T, T...'"
//@ diag: 40:23 error "literal operator template cannot have any parameters"
//@ diag: 41:29 warning "string literal operator templates are a GNU extension"
//@ diag: 42:5 error "non-template literal operator must have one or two parameters"
//@ diag: 43:20 error "invalid literal operator parameter type 'double', did you mean 'long double'?"
//@ diag: 44:19 error "invalid literal operator parameter type 'const volatile char *', did you mean 'const char *'?"
//@ diag: 45:13 error "string literal after 'operator' must be '\"\"'"
//@ diag: 46:14 error "string literal after 'operator' cannot have an encoding prefix"
//@ diag: 47:31 error "invalid literal operator parameter type 'long', did you mean 'unsigned long'?"
//@ diag: 48:18 error "invalid literal operator parameter type 'int *', did you mean 'const char *'?"
//@ diag: 49:31 error "parameter of literal operator must have type 'unsigned long long', 'long double', 'char', 'wchar_t', 'char16_t', 'char32_t', or 'const char *'"
//@ diag: 50:19 warning "user-defined literal suffixes containing '__' are reserved"
//@ diag: 51:1 error "template parameter list for literal operator must be either 'char...' or 'typename T, T...'"
typedef unsigned long size_t;
int operator"" _a(unsigned long long);
int operator""_b(long double);
int operator""xy(const char*);
int operator""sv(const char*, size_t);
int operator""_c(int);
int operator""_d(const char*, int);
int operator""_e(const char*, size_t, int);
int operator""_f(unsigned long long = 0);
struct S { int operator""_g(char); };
extern "C" int operator""_h(char);
template<char...> int operator""_i();
template<class T> int operator""_j();
template<char...> int operator""_k(char);
template<class C, C...> int operator""_l();
int operator""_m();
float operator""_n(double);
int operator""_n2(const volatile char*, size_t);
int operator"" "x" _o(char);
int operator u"" _p(char);
int operator""_q(const char*, long);
int operator""_s(int*);
struct T {}; int operator""_v(T);
namespace N { int operator""_x__y(char); }
template<int...> int operator""_z();
