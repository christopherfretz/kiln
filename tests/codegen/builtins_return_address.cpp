// WP-432: __builtin_return_address(N) lowers to `llvm.returnaddress(i32 N)`
// (clang CGBuiltin BI__builtin_return_address), alongside frame_address.

void *f() { return __builtin_return_address(0); }
void *g() { return __builtin_return_address(1); }
void *h() { return __builtin_frame_address(0); }
template <unsigned N> void *t() { return __builtin_return_address(N); }
void *u() { return t<2>(); }
