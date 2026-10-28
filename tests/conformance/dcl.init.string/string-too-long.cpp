// [dcl.init.string]/2: too many characters for the array
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:13 error "initializer-string for char array is too long, array size is 2 but initializer has size 4 (including the null terminating character)"
char s[2] = "abc";
