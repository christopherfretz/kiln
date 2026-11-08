// [lex.phases]/1.1: an input byte sequence that is not valid UTF-8 is
// accepted in a comment (line or block) and is an error elsewhere, here in
// an identifier, as clang.
//@ expect: compile-fail
//@ diag: 8:7 error "source file is not valid UTF-8"
/* block ¨ comment
   on two lines ÿþ */ // and a line comment Ã
 int x¨y = 0;
