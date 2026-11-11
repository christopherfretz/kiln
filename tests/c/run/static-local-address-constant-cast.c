// C23 6.7.11p4, 6.6p9-p10: an initializer of an object with static storage
// duration is a constant expression; an address constant may be formed
// with casts (pointer to pointer, integer to pointer, pointer to integer).
// Such a block-scope static is initialized before program startup, not
// when control reaches it, so a jump past its declaration (6.8.7.2p1)
// still sees the value, and no guard variable or C++ runtime is needed.
//@ expect: run
//@ xfail(kiln): WP-1286 follow-up, an initializer with a pointer cast (object to object pointer, integer to pointer, pointer to integer) does not fold (the Oven's cast leaves a pointer that points at nothing), so the static is initialized dynamically on first pass under a __cxa_guard, which a jump skips and a C link cannot resolve (sqlite3.c trimFunc, sqlite3Initialize)
//@ stdout: 32 5 1 1 1
int printf(const char *, ...);
typedef unsigned char u8;
typedef long iptr;
struct S { int a, b; } gs;
int g;
static int one(void) { return 1; }
struct FD { void *data; int (*fn)(void); const char *name; };
int main(void) {
  goto skip;
  static u8 *const sp[] = { (u8 *)" " };
  static const struct FD fd[] = { { (void *)(iptr)5, one, "one" } };
  static char *mp = (char *)&gs.b;
  static iptr ip = (iptr)&g;
skip:
  if (!sp[0] || !mp || !ip) {
    printf("not initialized\n");
    return 1;
  }
  printf("%d %ld %d %d %d\n", sp[0][0], (long)(iptr)fd[0].data,
         mp == (char *)&gs + sizeof(int), ip == (iptr)&g, fd[0].fn());
  return 0;
}
