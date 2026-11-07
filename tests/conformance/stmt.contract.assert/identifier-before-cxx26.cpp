// [lex.key]/1: before C++26 `contract_assert` is an ordinary identifier.
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
int contract_assert = 0;
int f() { return contract_assert; }
