// [stmt.if]/4: an always-inline function's `if consteval` called at run
// time takes the else branch however the call is inlined or folded; in a
// constant expression it takes the compound statement (WP-588).
//@ expect: run
//@ stdout: 8 8 1 7
extern "C" int printf(const char *, ...);

[[gnu::always_inline]] constexpr inline int ai() { if consteval { return 7; } else { return 8; } }
[[gnu::always_inline]] inline int ai2(int x) {
  if !consteval { return x + 1; } else { return x; }
}

int main(int argc, char **) {
  constexpr int c = ai();
  printf("%d %d %d %d\n", ai(), argc ? ai() : 0, ai2(0), c);
}
