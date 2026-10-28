//@ expect: pp
// Headers not ending in a newline: the last token of the header is not
// glued to the first token after the #include, and a function-like macro
// name at the end of a header does not consume a `(` from the includer.
// An invocation that closes on the header's last byte is complete.
#include "Inputs/no-newline.h"
next_line
#include "Inputs/name-at-eof.h"
(1)
G(2)
#include "Inputs/invocation-at-eof.h"
after_invocation_header H(3)
