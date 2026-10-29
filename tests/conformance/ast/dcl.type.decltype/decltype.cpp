// [dcl.type.decltype] and placeholder return types ([dcl.spec.auto]).
int x;
int &rx = x;
decltype(x) a = 1;
decltype((x)) b = x;
decltype(rx) c = x;
decltype(x + 1) d = 2;
auto f() { return 3; }
auto g() -> long { return 4; }
decltype(auto) h() { return (x); }
auto k() -> decltype(x * 2) { return x * 2; }
