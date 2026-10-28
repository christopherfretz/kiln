//@ expect: pp
//@ std: c++14
// In an ISO C++14 mode trigraphs are replaced in phase 1, before line
// splicing, so ??/ followed by a newline is a line splice and ??/ inside a
// literal is a backslash.
??=define X 1
X a??(b??) c ??< ??> ??! ??' ??- "??/"" '??/''
??=define Y 2 ??/
+ 3
Y
