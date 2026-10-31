// [temp.variadic]: a fold over a deduced pack at run time.
//@ expect: run
template <class... Ts> int sum(Ts... ts) { return (ts + ... + 0); }
int main() { return sum(1, 2, 3) == 6 && sum() == 0 ? 0 : 1; }
