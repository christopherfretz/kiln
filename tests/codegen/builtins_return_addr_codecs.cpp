// WP-1288: `__builtin_extract_return_addr` and `__builtin_frob_return_addr`
// are the identity on AArch64 and x86-64 (clang's default
// `decodeReturnAddress` / `encodeReturnAddress`); `__builtin___clear_cache`
// lowers to `llvm.clear_cache` (clang CGBuiltin BI__builtin___clear_cache).

void *dec(void *p) { return __builtin_extract_return_addr(p); }
void *enc(void *p) { return __builtin_frob_return_addr(p); }
void *ra() { return __builtin_extract_return_addr(__builtin_return_address(0)); }
void flush(char *b, char *e) { __builtin___clear_cache(b, e); }
