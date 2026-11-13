// Ends with #pragma weak naming a function declared only in the main
// file (pending), and one the prefix declared.
#pragma weak pfx_later
extern "C" int pfx_now();
#pragma weak pfx_now
