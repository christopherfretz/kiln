//@ expect: pp
// #embed (C++26 P1967, an extension in clang 19's C++ mode) with the
// standard parameters limit, prefix, suffix and if_empty, and __has_embed.
int all[] = {
#embed "Inputs/embed6.bin"
};
int three[] = {
#embed "Inputs/embed6.bin" limit(3)
};
#define LIM 2
int macro_limit[] = {
#embed "Inputs/embed6.bin" limit(LIM)
};
int wrapped[] = {
#embed "Inputs/embed6.bin" limit(2) prefix(100, ) suffix(, 200) if_empty(-1)
};
int empty_file[] = {
#embed "Inputs/empty.bin" prefix(1,) suffix(,2) if_empty(42)
};
int zero_limit[] = {
#embed "Inputs/embed6.bin" limit(0) if_empty(7, 8)
};
#if __has_embed("Inputs/embed6.bin") == __STDC_EMBED_FOUND__
found;
#endif
#if __has_embed("Inputs/empty.bin") == __STDC_EMBED_EMPTY__
empty;
#endif
#if __has_embed("Inputs/no-such-file.bin") == __STDC_EMBED_NOT_FOUND__
not_found;
#endif
