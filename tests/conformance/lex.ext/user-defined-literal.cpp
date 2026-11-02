// [lex.ext]: a user-defined literal needs a literal operator
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:11 error "no matching literal operator for call to 'operator\"\"_km' with argument of type 'unsigned long long' or 'const char *', and no matching literal operator template"
//@ diag: 11:15 error "no matching literal operator for call to 'operator\"\"_km' with argument of type 'long double' or 'const char *', and no matching literal operator template"
//@ diag: 12:20 error "no matching literal operator for call to 'operator\"\"_km' with arguments of types 'const char *' and '__size_t' (aka 'unsigned long'), and no matching literal operator template"
//@ diag: 13:13 error "no matching literal operator for call to 'operator\"\"_km' with argument of type 'char'"
//@ diag: 14:28 error "no matching literal operator for call to 'operator\"\"_km' with arguments of types 'const wchar_t *' and '__size_t' (aka 'unsigned long'), and no matching literal operator template"
//@ requires: clang>=22
int a = 12_km;
double b = 1.5_km;
const char *c = "x"_km;
char d = 'x'_km;
const wchar_t *e = L"x" "y"_km;
