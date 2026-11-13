// Ends with '#pragma weak alias = target' naming a target declared only
// in the main file (pending), and one the prefix defined.
extern "C" int pfx_target() { return 1; }
#pragma weak pfx_alias = pfx_target
#pragma weak pfx_alias_later = pfx_later
