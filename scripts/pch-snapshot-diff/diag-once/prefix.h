// Warnings kiln gives once (per entity, location or instantiation
// context), first triggered in the prefix and triggered again by the
// main file.
static int internal_undefined();
inline int pfx_uses_internal() { return internal_undefined(); }

[[deprecated]] inline int old_api() { return 1; }
template <class T> int pfx_calls_old(T) { return old_api(); }
inline int pfx_inst = pfx_calls_old(1);

template <class T> int pfx_null(T) { return *(T *)0; }
inline int pfx_null_inst = pfx_null(1);
