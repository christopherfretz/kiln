// [basic.fundamental]/7: plain char is signed on x86-64 by default;
// -funsigned-char overrides the target (and clang's driver then predefines
// __CHAR_UNSIGNED__), while -fsigned-char after it wins (last flag wins).
//@ args: --target=x86_64-unknown-linux-gnu -fsigned-char -funsigned-char

#ifndef __CHAR_UNSIGNED__
#error "-funsigned-char should predefine __CHAR_UNSIGNED__"
#endif

static_assert(char(-1) > 0);
static_assert(!__is_signed(char));
static_assert(int(char('\x80')) == 128);
static_assert(!__is_same(char, unsigned char));

// The usual arithmetic conversions see an unsigned char promoted to int.
static_assert(char(0xff) + 1 == 256);
