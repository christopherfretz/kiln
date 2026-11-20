// [cpp.cond]/5, /12, [cpp.embed.param.limit]: the `limit` parameter of a
// `__has_embed` inside `#if` is an integral constant expression evaluated
// with the full `#if` rules: character literals, `?:`, `intmax_t` /
// `uintmax_t` conversions (with clang's warning), alternative tokens. (kiln once used a
// reduced grammar there and rejected these.)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-c23-extensions
//@ diag: 14:35 warning "left side of operator converted from negative value to unsigned: -1 to 18446744073709551615"
#define ZERO 0
#if __has_embed(__FILE__ limit('a' - 'a')) != __STDC_EMBED_EMPTY__
#error "char literal"
#endif
#if __has_embed(__FILE__ limit(-1 < 0u ? 0 : 1)) != __STDC_EMBED_FOUND__
#error "uintmax_t conversion"
#endif
#if __has_embed(__FILE__ limit(ZERO bitor (1 and 0))) != __STDC_EMBED_EMPTY__
#error "alternative tokens"
#endif
#if __has_embed(__FILE__ limit(u8'\x01' * 0x10ULL >> 4)) != __STDC_EMBED_FOUND__
#error "u8 literal and suffix"
#endif
int main() {}
