// Clang extension (C11 6.7.3 in C++): the bare `_Atomic` type qualifier
// names _Atomic of the unqualified type with the cv-qualifiers outside it;
// its objects behave as _Atomic(T) ones.
_Atomic int a;
int _Atomic b;
const _Atomic int c = 1;
typedef const int CI;
_Atomic CI d = 2;
_Atomic int arr[2];
int f() { _Atomic long x = 3; x += a; b = x; return arr[1]++; }
