// Clang extension: an _Atomic object takes no braced initializer (clang
// InitListChecker: illegal initializer type); = and () initialize it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "illegal initializer type '_Atomic(int)'"
//@ diag: 10:18 error "illegal initializer type '_Atomic(int)'"
//@ diag: 11:15 error "illegal initializer type '_Atomic(int)'"
//@ diag: 12:26 error "illegal initializer type '_Atomic(int)'"
_Atomic(int) a{5};
_Atomic(int) b = {5};
_Atomic(int) c{};
struct S { _Atomic(int) m{1}; };
_Atomic(int) d(5);
_Atomic(int) e = 5;
