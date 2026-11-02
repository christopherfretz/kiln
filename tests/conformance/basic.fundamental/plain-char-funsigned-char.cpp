// [basic.fundamental]/7: whether plain char is signed is implementation-
// defined; -funsigned-char makes it unsigned on every target (clang: the
// driver flag also sets __CHAR_UNSIGNED__ accordingly), while char stays a
// type distinct from both signed char and unsigned char.
//@ expect: run
//@ args: -funsigned-char
//@ stdout: 255 255 1 1
extern "C" int printf(const char*, ...);

#if defined(__CHAR_UNSIGNED__) != 1
#error "__CHAR_UNSIGNED__ does not match -funsigned-char"
#endif

// [cpp.cond]/12: character literals in #if take plain char's signedness.
#if '\xff' < 0
#error "#if sees the wrong char signedness under -funsigned-char"
#endif

#ifdef __CHAR_UNSIGNED__
constexpr int char_unsigned_macro = 1;
#else
constexpr int char_unsigned_macro = 0;
#endif

static_assert((char(-1) < 0) == false);
static_assert(int(char(0xff)) == 255);
static_assert(__is_signed(char) == false);
static_assert(!__is_same(char, signed char) && !__is_same(char, unsigned char));
static_assert(sizeof(char) == 1);

constexpr int widen(char c) { return c; }
static_assert(widen('\xff') == 255);

int f(signed char) { return 0; }
int f(unsigned char) { return 0; }
int f(char) { return 1; }

int main() {
  volatile char c = '\xff';
  char d = c;
  int i = d;
  printf("%d %d %d %d\n", i, widen(d), f(d), char_unsigned_macro);
}
