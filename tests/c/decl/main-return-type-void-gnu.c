// C23 5.1.2.3.2p1 (program startup): main is defined with return type
// int; a hosted implementation may accept other forms. clang accepts a
// void main with a warning in GNU modes (-Wmain-return-type) and rejects
// it in ISO C modes and in C++ ([basic.start.main]p3).
// gcc.c-torture: execute/930408-1.c.
//@ expect: compile-ok
//@ args: -std=gnu17
//@ strict-diags
//@ diag: 11:1 warning "return type of 'main' is not 'int'"
//@ diag: 11:1 note "change return type to 'int'"
void main(void) {}
