export module mod_used_inline;
export inline __attribute__((used)) int one() { return 1; }
export inline __attribute__((used)) int pick(int) { return 2; }
export inline __attribute__((used)) int pick(long) { return 3; }
export inline __attribute__((used)) int in_sizeof() { return 4; }
export inline __attribute__((used)) int unnamed() { return 5; }
export inline int plain() { return 6; }
export namespace ns {
inline __attribute__((used)) int deep() { return 7; }
}
