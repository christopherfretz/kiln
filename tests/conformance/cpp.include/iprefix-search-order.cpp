// [cpp.include]/2: `-iprefix <p>` sets the prefix of the later
// `-iwithprefix <d>` (directory `<p><d>`, after the system directories and
// before every `-idirafter`, even one written earlier) and
// `-iwithprefixbefore <d>` (in the `-I` list, after every `-I`, even one
// written later); before any `-iprefix` the prefix is empty (clang
// ParseHeaderSearchArgs). WP-2822.
//@ expect: pp
//@ args: -iwithprefixbefore tests/conformance/cpp.include/Inputs/prefix/e -iwithprefixbefore pb -iprefix tests/conformance/cpp.include/Inputs/prefix/ -idirafter tests/conformance/cpp.include/Inputs/prefix/da -iwithprefix pa -iwithprefixbefore pb -I tests/conformance/cpp.include/Inputs/prefix/i
#include <e.h>
#include <order.h>
#include <before.h>
#include <after.h>
