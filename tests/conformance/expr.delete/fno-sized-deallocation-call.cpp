// [basic.stc.dynamic.deallocation]/2: under -fno-sized-deallocation
// `operator delete(void*, std::size_t)` is not declared implicitly.
// WP-808.
//@ expect: compile-fail
//@ args: -fno-sized-deallocation
//@ diag: 7:19 error "no matching function for call to 'operator delete'"
void k(void* p) { ::operator delete(p, 4); }
