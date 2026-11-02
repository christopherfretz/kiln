// [over.literal]: literal operator declarations (clang CheckLiteralOperatorDeclaration)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:15 error "invalid suffix on literal; C++11 requires a space between literal and identifier"
//@ diag: 34:5 warning "user-defined literal suffixes not starting with '_' are reserved; no literal will invoke this operator"
//@ diag: 35:5 warning "user-defined literal suffixes not starting with '_' are reserved"
//@ diag: 36:18 error "invalid literal operator parameter type 'int', did you mean 'unsigned long long'?"
//@ diag: 37:31 error "invalid literal operator parameter type 'int', did you mean '__size_t' (aka 'unsigned long')?"
//@ diag: 38:5 error "non-template literal operator must have one or two parameters"
//@ diag: 39:39 error "literal operator cannot have a default argument"
//@ diag: 40:16 error "literal operator 'operator\"\"_g' must be in a namespace or global scope"
//@ diag: 41:16 error "literal operator must have C++ linkage"
//@ diag: 41:1 note "extern \"C\" language linkage specification begins here"
//@ diag: 43:1 error "template parameter list for literal operator must be either 'char...' or 'typename T, T...'"
//@ diag: 44:23 error "literal operator template cannot have any parameters"
//@ diag: 45:29 warning "string literal operator templates are a GNU extension"
//@ diag: 46:5 error "non-template literal operator must have one or two parameters"
//@ diag: 47:20 error "invalid literal operator parameter type 'double', did you mean 'long double'?"
//@ diag: 48:19 error "invalid literal operator parameter type 'const volatile char *', did you mean 'const char *'?"
//@ diag: 49:13 error "string literal after 'operator' must be '\"\"'"
//@ diag: 50:14 error "string literal after 'operator' cannot have an encoding prefix"
//@ diag: 51:31 error "invalid literal operator parameter type 'long', did you mean '__size_t' (aka 'unsigned long')?"
//@ diag: 52:18 error "invalid literal operator parameter type 'int *', did you mean 'const char *'?"
//@ diag: 53:31 error "parameter of literal operator must have type 'unsigned long long', 'long double', 'char', 'wchar_t', 'char16_t', 'char32_t', or 'const char *'"
//@ diag: 54:19 warning "user-defined literal suffixes containing '__' are reserved"
//@ diag: 55:1 error "template parameter list for literal operator must be either 'char...' or 'typename T, T...'"
//@ diag: 32:16 warning "identifier '_a' preceded by whitespace in a literal operator declaration is deprecated"
//@ diag: 49:20 warning "identifier '_o' preceded by whitespace in a literal operator declaration is deprecated"
//@ diag: 50:18 warning "identifier '_p' preceded by whitespace in a literal operator declaration is deprecated"
//@ requires: clang>=22
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
