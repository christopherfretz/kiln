// [lex.phases]/1.1: an input byte sequence that is not valid UTF-8 is
// implementation-defined; clang accepts it in a comment (-Winvalid-utf8
// is off by default) and so does GCC. The comment below holds the
// byte 0xA8 alone, as Boost.Spirit karma regression_real_0.cpp:20 does.
//@ expect: run
//@ xfail(kiln): WP-787 follow-up, every invalid UTF-8 sequence in the file is an error, comments included (kiln_pp preprocessor.rs push_file)
//@ stdout: ok
extern "C" int printf(const char*, ...);
// prints ¨inf¨
int main() { printf("ok\n"); }
