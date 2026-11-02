//@ expect: ast
//@ requires: clang>=22
const char *s = "abc" "def";
const char *t = "x"
                "y"
                "z";
const char u[] = "one" "two";
unsigned long n = sizeof("ab" "cd");
