// C17 6.4.4.4p10: an integer character constant has type int, so
// sizeof('a') == sizeof(int); a multi-character constant too. C17
// 6.4.5p6: a character string literal is an array of char (not const).
//@ expect: compile-ok
//@ strict-diags
_Static_assert(sizeof('a') == sizeof(int), "char constant is int");
_Static_assert(sizeof('\0') == 4, "char constant is int");
_Static_assert(_Generic('a', int: 1, default: 0), "type is int");
_Static_assert(_Generic("ab", char *: 1, default: 0), "string decays to char *");
_Static_assert(sizeof("abc") == 4, "char[4]");
char *p = "writable type";
char a[] = "xyz";
_Static_assert(sizeof a == 4, "");
