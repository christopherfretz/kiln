//@ expect: pp
// Backslash-newline line splices inside every kind of token: identifiers,
// numbers, punctuators, string and character literals, directive names,
// and immediately before end of file in an included header.
#def\
ine SPL\
ICED val\
ue
SPLICED SPL\
ICED 12\
34 0x\
1F +\
+ -\
> <\
<= "str\
ing" 'c\
' u8\
"pref" L\
'w'
/\
/ comment via spliced slashes
/\
* block comment via splice *\
/ after_comment;
#include "Inputs/splice-at-eof.h"
after_include;
