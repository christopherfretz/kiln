//! A friend's written form (spec/ast.md §23; WP-3164, the WP-3124 design).
//!
//! A `Friend` declaration records what it befriends ([`FriendTarget`]).
//! When the friend is written with parts the target cannot hold (a
//! qualified declarator-id, a template-id, a special member name, a
//! function kept with its type only, a qualified class friend with a
//! template-head of its own), the declaration also has one
//! [`FriendForm`] row: the qualifier and the template arguments as
//! written (their expressions included), the friend's own template-heads
//! and trailing requires-clause, and what the written name refers to
//! ([`FriendForm::entity`]). Rows live in [`DeclArena::friend_forms`] in
//! declaration order; `ColdDeclData::friend_form` maps a `Friend` to its
//! row. Sema reads its own friend matching state from the row.

use std::fmt::Write as _;

use kiln_source::{FileId, Loc, SourceManager};

use crate::Ast;
use crate::decl::{DeclArena, DeclKind, FriendTarget};
use crate::ids::{ArgListId, DeclId, ExprId, ListId, NnsId, TemplateParamListId};
use crate::names::DeclName;
use crate::slot::AlreadySet;

/// What kind of friend a row describes (spec/ast.md §23.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
#[repr(u8)]
pub enum FriendFormKind {
    /// A function or function template named by a qualified name
    /// (`friend void Bq<N>::g();`, `friend void N::f(T);`).
    Function,
    /// A function template specialization ([temp.friend]/1.3:
    /// `friend void h<int>();`, `friend void f<>(T);`).
    Specialization,
    /// A constructor (`friend Z<T>::Z();`).
    Constructor,
    /// A destructor (`friend Z<T>::~Z();`).
    Destructor,
    /// A conversion function (`friend Z<T>::operator int();`).
    Conversion,
    /// A class with a template-head of its own
    /// (`template <class U> friend class A<U>::In;`); its written
    /// qualifier is the type sugar of the target.
    Class,
}

impl FriendFormKind {
    /// The dump's spelling.
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Specialization => "specialization",
            Self::Constructor => "constructor",
            Self::Destructor => "destructor",
            Self::Conversion => "conversion",
            Self::Class => "class",
        }
    }
}

/// The flags of a [`FriendForm`] (spec/ast.md §23.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, kiln_snap::Snap)]
pub struct FriendFormFlags(u8);

impl FriendFormFlags {
    /// No flag.
    pub const NONE: Self = Self(0);
    /// The qualifier begins with the global `::` (`friend void ::N::k();`).
    pub const GLOBAL: Self = Self(1);
    /// The template-id after the qualifier is introduced by `template`
    /// (`friend void Bq<8>::template t<1>();`).
    pub const TEMPLATE_KW: Self = Self(2);
    /// A friend of every specialization ([temp.friend]/5): the friend's
    /// own template-head deduces the qualifier's template arguments
    /// (`template <class U> friend void A<U>::f();`).
    pub const EVERY_SPEC: Self = Self(4);
    /// The friend is matched per specialization of its class: its
    /// qualifier, template arguments, own template-head or type depend on
    /// the class's template parameters. [`FriendForm::entity`] is then a
    /// candidate, not the referent.
    pub const DEPENDENT: Self = Self(8);
    /// The friend was accepted with clang's `-Wunsupported-friend`
    /// warning (a dependent nested name specifier for a friend).
    pub const UNSUPPORTED: Self = Self(16);

    /// The raw bits.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Whether every flag of `other` is set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for FriendFormFlags {
    type Output = Self;
    fn bitor(self, o: Self) -> Self {
        Self(self.0 | o.0)
    }
}

impl std::ops::BitOrAssign for FriendFormFlags {
    fn bitor_assign(&mut self, o: Self) {
        self.0 |= o.0;
    }
}

/// One friend's written form (40 bytes, a `size_of` test; spec/ast.md
/// §23.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug, kiln_snap::Snap)]
pub struct FriendForm {
    /// What kind of friend.
    pub kind: FriendFormKind,
    /// [`FriendFormFlags`].
    pub flags: FriendFormFlags,
    /// The befriended name as written (`g`, `h`, `~Z`, `operator int`;
    /// a constructor's and destructor's name is the qualifier's class's,
    /// a class friend's its identifier).
    pub name: DeclName,
    /// Where the name was written.
    pub name_loc: Loc,
    /// The nested-name-specifier as written: a `Nns::Type` component
    /// keeps its sugar, so `Bq<sizeof(sizeof(f(T())))>::` holds the
    /// expression, not the converted `8`. None when unqualified.
    pub qualifier: Option<NnsId>,
    /// Where the qualifier begins (invalid when unqualified).
    pub qualifier_loc: Loc,
    /// The template-id's arguments as written: `Some(ArgListId::EMPTY)`
    /// for `f<>`, None for no template-id.
    pub args: Option<ArgListId>,
    /// What the written name refers to (spec/ast.md §23.3): None when the
    /// target is a declaration that is the entity, or when nothing is
    /// known; with [`FriendFormFlags::DEPENDENT`] a candidate.
    pub entity: Option<DeclId>,
    /// The function templates lookup of a template-id's name found
    /// ([temp.friend]/1.3), first declarations; `ListId::EMPTY` otherwise.
    pub found: ListId,
    /// The friend's own template-heads, outermost first
    /// (`TemplateParamListId`s; the inner one of `template <class T>
    /// template <class U> friend void A<T>::g(U);` second).
    pub heads: ListId,
    /// The trailing requires-clause.
    pub requires: Option<ExprId>,
}

impl FriendForm {
    /// A row of `kind` for the name `name` written at `name_loc`, with no
    /// other part.
    pub const fn new(kind: FriendFormKind, name: DeclName, name_loc: Loc) -> Self {
        Self {
            kind,
            flags: FriendFormFlags::NONE,
            name,
            name_loc,
            qualifier: None,
            qualifier_loc: Loc::INVALID,
            args: None,
            entity: None,
            found: ListId::EMPTY,
            heads: ListId::EMPTY,
            requires: None,
        }
    }
}

/// [`Ast::friend_form`]: a `Friend`'s written form, absent, or not known.
#[derive(Clone, Copy, Debug)]
pub enum FriendFormAnswer<'a> {
    /// The row.
    Row(FriendFormView<'a>),
    /// The friend has no written parts beyond its target (unqualified, no
    /// template-id: `friend void f(int);`, `friend class C;`).
    None,
    /// The declaration was read from a module image, which does not carry
    /// rows (phase 1): whether it had one is not known.
    NotCarried,
}

impl<'a> FriendFormAnswer<'a> {
    /// The row, if any.
    pub fn row(self) -> Option<FriendFormView<'a>> {
        match self {
            Self::Row(v) => Some(v),
            Self::None | Self::NotCarried => None,
        }
    }
}

/// A borrowed row with the `Ast` it belongs to.
#[derive(Clone, Copy, Debug)]
pub struct FriendFormView<'a> {
    ast: &'a Ast,
    friend: DeclId,
    row: &'a FriendForm,
}

impl<'a> FriendFormView<'a> {
    /// The `Friend` declaration.
    pub fn friend_decl(self) -> DeclId {
        self.friend
    }

    /// The row's fields.
    pub fn form(self) -> &'a FriendForm {
        self.row
    }

    /// [`FriendForm::kind`].
    pub fn kind(self) -> FriendFormKind {
        self.row.kind
    }

    /// [`FriendForm::flags`].
    pub fn flags(self) -> FriendFormFlags {
        self.row.flags
    }

    /// Whether flag `f` is set.
    pub fn has(self, f: FriendFormFlags) -> bool {
        self.row.flags.contains(f)
    }

    /// [`FriendForm::name`].
    pub fn name(self) -> DeclName {
        self.row.name
    }

    /// [`FriendForm::name_loc`].
    pub fn name_loc(self) -> Loc {
        self.row.name_loc
    }

    /// [`FriendForm::qualifier`].
    pub fn qualifier(self) -> Option<NnsId> {
        self.row.qualifier
    }

    /// [`FriendForm::qualifier_loc`].
    pub fn qualifier_loc(self) -> Loc {
        self.row.qualifier_loc
    }

    /// [`FriendForm::args`].
    pub fn args(self) -> Option<ArgListId> {
        self.row.args
    }

    /// What the written name refers to: the row's entity, else (not
    /// `DEPENDENT`) the target when it is a declaration.
    pub fn entity(self) -> Option<DeclId> {
        if self.row.entity.is_some() || self.has(FriendFormFlags::DEPENDENT) {
            return self.row.entity;
        }
        match self.ast.decls.friend_target(self.friend) {
            FriendTarget::Decl(t) => Some(t),
            FriendTarget::Type { .. } => None,
        }
    }

    /// [`FriendForm::found`].
    pub fn found(self) -> impl ExactSizeIterator<Item = DeclId> + 'a {
        self.ast.lists.iter::<DeclId>(self.row.found)
    }

    /// [`FriendForm::heads`], outermost first.
    pub fn heads(self) -> impl ExactSizeIterator<Item = TemplateParamListId> + 'a {
        self.ast.lists.iter::<TemplateParamListId>(self.row.heads)
    }

    /// The outermost own head.
    pub fn head(self) -> Option<TemplateParamListId> {
        self.ast.lists.at::<TemplateParamListId>(self.row.heads, 0)
    }

    /// The parameters of head `h` as declarations (their defaults and
    /// type-constraints are theirs; `TemplateTables`).
    pub fn head_params(self, h: TemplateParamListId) -> impl ExactSizeIterator<Item = DeclId> + 'a {
        let l = self.ast.templates.param_list(h).params;
        self.ast.lists.iter::<DeclId>(l)
    }

    /// The requires-clause of head `h`.
    pub fn head_requires(self, h: TemplateParamListId) -> Option<ExprId> {
        self.ast.templates.param_list(h).requires
    }

    /// [`FriendForm::requires`].
    pub fn requires(self) -> Option<ExprId> {
        self.row.requires
    }
}

/// [`ColdDeclData::friend_form`](crate::decl::ColdDeclData::friend_form)'s
/// value for a `Friend` read from a module image (spec/ast.md §23.4).
pub const NOT_CARRIED: u32 = u32::MAX;

impl DeclArena {
    /// The row of `Friend` `fd`: its index, `Some(NOT_CARRIED)`, or none.
    pub fn friend_form_index(&self, fd: DeclId) -> Option<u32> {
        self.cold.friend_form.get(fd).copied()
    }

    /// The row of `Friend` `fd`, when it has one.
    pub fn friend_form_row(&self, fd: DeclId) -> Option<&FriendForm> {
        let i = self.friend_form_index(fd).filter(|&i| i != NOT_CARRIED)?;
        self.friend_forms.get(i as usize)
    }

    /// Every row with its `Friend`, in declaration order.
    pub fn friend_forms(&self) -> impl ExactSizeIterator<Item = (DeclId, &FriendForm)> + '_ {
        self.friend_form_decls.iter().copied().zip(self.friend_forms.iter())
    }

    /// Records the row of `Friend` `fd` (write-once).
    ///
    /// # Errors
    /// When `fd` has a row, or is marked not carried, already.
    ///
    /// # Panics
    /// If `fd` is not a `Friend`, or more than `u32::MAX - 1` rows.
    pub fn set_friend_form(&mut self, fd: DeclId, row: FriendForm) -> Result<(), AlreadySet> {
        assert_eq!(self.get(fd).kind(), DeclKind::Friend, "a friend form of a non-friend");
        let i = u32::try_from(self.friend_forms.len()).expect("friend form rows");
        assert!(i != NOT_CARRIED, "friend form rows");
        self.cold.friend_form.insert(fd, i)?;
        self.friend_forms.push(row);
        self.friend_form_decls.push(fd);
        Ok(())
    }

    /// Adds flags `f` to the row of `fd` (monotone), when it has one.
    pub fn add_friend_form_flags(&mut self, fd: DeclId, f: FriendFormFlags) {
        if let Some(i) = self.friend_form_index(fd).filter(|&i| i != NOT_CARRIED)
            && let Some(r) = self.friend_forms.get_mut(i as usize)
        {
            r.flags |= f;
        }
    }

    /// Sets the entity of the row of `fd` when it has none (a
    /// [temp.friend]/5 friend's, found once it is registered).
    pub fn set_friend_form_entity(&mut self, fd: DeclId, e: Option<DeclId>) {
        if let Some(i) = self.friend_form_index(fd).filter(|&i| i != NOT_CARRIED)
            && let Some(r) = self.friend_forms.get_mut(i as usize)
            && r.entity.is_none()
        {
            r.entity = e;
        }
    }

    /// Marks `Friend` `fd`, read from a module image, as one whose row is
    /// not carried ([`FriendFormAnswer::NotCarried`]).
    pub fn set_friend_form_not_carried(&mut self, fd: DeclId) {
        let _ = self.cold.friend_form.insert(fd, NOT_CARRIED);
    }
}

impl Ast {
    /// The written form of `Friend` `fd` (spec/ast.md §23).
    pub fn friend_form(&self, fd: DeclId) -> FriendFormAnswer<'_> {
        match self.decls.friend_form_index(fd) {
            None => FriendFormAnswer::None,
            Some(NOT_CARRIED) => FriendFormAnswer::NotCarried,
            Some(i) => match self.decls.friend_forms.get(i as usize) {
                Some(row) => FriendFormAnswer::Row(FriendFormView { ast: self, friend: fd, row }),
                None => FriendFormAnswer::None,
            },
        }
    }

    /// The `-fkiln-dump-friend-forms` dump (spec/ast.md §23.5): one line
    /// per row, in declaration order.
    pub fn dump_friend_forms(&self, sm: &SourceManager, main: Option<FileId>) -> String {
        let names = crate::dump::AstNames::new(self);
        let pos = |loc: Loc| -> String {
            match sm.line_col(sm.spelling_loc(loc)) {
                Some(lc) if Some(lc.file) == main => format!("{}:{}", lc.line, lc.col),
                Some(lc) => {
                    let n = sm.file_name(lc.file);
                    let base = n.rsplit(['/', '\\']).next().unwrap_or(n);
                    format!("{base}:{}:{}", lc.line, lc.col)
                }
                None => "?".to_string(),
            }
        };
        let decl = |d: DeclId, out: &mut String| {
            let mut s = String::new();
            crate::types::TypeNames::write_decl_name(&names, d, true, &mut s);
            if s.is_empty() {
                s.push_str("(anonymous)");
            }
            let _ = write!(out, "{s}@{}", pos(self.decls.get(d).loc()));
        };
        let mut out = String::new();
        for (fd, r) in self.decls.friend_forms() {
            let _ =
                write!(out, "{} FriendForm {}", pos(self.decls.get(fd).loc()), r.kind.spelling());
            if let Some(q) = r.qualifier {
                let mut s = String::new();
                crate::types::write_nns(&self.types, &names, q, &mut s);
                let _ = write!(out, " qualifier '{s}'");
            }
            let mut n = String::new();
            names.write_decl_name_value(r.name, &mut n);
            let _ = write!(out, " name '{n}'@{}", pos(r.name_loc));
            if let Some(a) = r.args {
                let mut s = String::new();
                crate::types::write_arg_list(&self.types, &names, a, &mut s);
                if s.is_empty() {
                    s.push_str("<>");
                }
                let _ = write!(out, " args {s}");
            }
            let heads = self.lists.len(r.heads);
            if heads > 0 {
                let _ = write!(out, " heads {heads}");
            }
            if r.requires.is_some() {
                out.push_str(" requires");
            }
            if let Some(e) = r.entity {
                out.push_str(" entity ");
                decl(e, &mut out);
            }
            if !self.lists.is_empty(r.found) {
                out.push_str(" found [");
                for (k, f) in self.lists.iter::<DeclId>(r.found).enumerate() {
                    if k > 0 {
                        out.push_str(", ");
                    }
                    decl(f, &mut out);
                }
                out.push(']');
            }
            for (f, s) in [
                (FriendFormFlags::GLOBAL, "global"),
                (FriendFormFlags::TEMPLATE_KW, "template"),
                (FriendFormFlags::EVERY_SPEC, "every-spec"),
                (FriendFormFlags::DEPENDENT, "dependent"),
                (FriendFormFlags::UNSUPPORTED, "unsupported"),
            ] {
                if r.flags.contains(f) {
                    out.push(' ');
                    out.push_str(s);
                }
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_is_40_bytes() {
        assert_eq!(size_of::<FriendForm>(), 40);
    }
}
