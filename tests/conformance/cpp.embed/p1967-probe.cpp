// [cpp.embed] (C++26, P1967R14): `#embed` expands to the comma-separated
// bytes of a resource, with `limit`, `prefix`, `suffix` and `if_empty`
// parameters, and `__has_embed` queries it. WP-185 feature probe. (clang 19
// calls `#embed` in C++ a C23 extension, hence -Wno-c23-extensions.)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-c23-extensions
constexpr unsigned char data[] = {
#embed "Inputs/p1967-data.txt"
};
static_assert(sizeof(data) == 4 && data[0] == 'k' && data[3] == 'n');
constexpr int two[] = {
#embed "Inputs/p1967-data.txt" limit(2) suffix(, 0)
};
static_assert(sizeof(two) / sizeof(int) == 3 && two[1] == 'i' && two[2] == 0);
#if __has_embed("Inputs/p1967-data.txt") != __STDC_EMBED_FOUND__
#error "__has_embed"
#endif
#if __has_embed("no-such-file.txt") != __STDC_EMBED_NOT_FOUND__
#error "__has_embed missing"
#endif
