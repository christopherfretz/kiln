// Ends with '#pragma redefine_extname' naming a function declared only in
// the main file (pending), and one the prefix declared.
extern "C" int pfx_now();
#pragma redefine_extname pfx_now pfx_now_renamed
#pragma redefine_extname pfx_later pfx_later_renamed
