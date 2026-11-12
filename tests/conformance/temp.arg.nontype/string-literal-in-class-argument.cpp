// [temp.arg.nontype]/6.2: a template argument of class type whose
// constituent pointer points to a string literal is not a permitted
// result of the converted constant expression, so the explicitly
// specified argument is invalid and `f` has no viable candidate. Found
// while writing the P4101R1 tests (WP-1328); clang 19/22 and GCC 16.1
// reject it.
//@ expect: compile-fail
//@ oracle: gcc
//@ xfail(kiln): accepts a string-literal pointer as a constituent of a class-type template argument (found by WP-1328, follow-up row pending)
//@ diag: 15:9 error "no matching function for call to 'f'"
//@ diag: 14:19 note "invalid explicitly-specified argument for template parameter 't'"

struct T { const char* p; };
template<T t> int f() { return 0; }
int x = f<T{"x"}>();
