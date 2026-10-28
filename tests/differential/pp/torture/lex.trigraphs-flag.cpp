//@ expect: pp
//@ std: c++26
//@ args: -trigraphs
// -trigraphs re-enables replacement in modes that otherwise ignore them.
??=define X 1
X a??(b??) "??=" ??/
continued
