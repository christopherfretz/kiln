//! The declaration image of a precompiled header (WP-1036, spec/driver.md
//! "Precompiled headers", spec/modules.md §11.11 "Precompiled headers").
//!
//! The producer writes the header's declarations as if the header were
//! the purview of a unit: every declaration of the translation unit in a
//! real file (the header and what it includes), owned by no module. The
//! image is a module image whose module name is empty and whose module
//! keyword, name and preamble words are [`NONE`]; a module name is never
//! empty, so neither reader accepts the other's image. Unlike a module's,
//! every function body is carried: a consumer emits the header's
//! definitions as textual inclusion does.
//!
//! The consumer builds the records into its translation unit before its
//! first token, in the global module (owner byte 0, the unit's own), so
//! the unit may redeclare, define, specialize and instantiate what the
//! header declares. The driver loads the image's file table into the
//! source manager first ([`PchDecls::files`]) and decides, before
//! preprocessing, whether the image or a replay of the header is used:
//! a malformed image found here is a hard error, never a partial build.

use kiln_ast::DeclId;
use kiln_source::{FileId, Loc};

use super::locs::{FileRec, Locs, validate};
use super::{Census, Image, ImageCensus, ModuleImage, NONE, Writer, decode, encode, gated_names};
use crate::Sema;

/// A decoded and checked precompiled-header image, ready to be built.
#[derive(Debug)]
pub struct PchDecls {
    img: Image,
    bases: Vec<u32>,
}

impl PchDecls {
    /// Decodes the image section `bytes` and checks its location records.
    ///
    /// # Errors
    /// Why the bytes are not a precompiled header's image.
    pub fn decode(bytes: &[u8]) -> Result<PchDecls, String> {
        let img = decode(bytes).map_err(|e| e.0)?;
        if !img.module.is_empty()
            || img.kw != NONE
            || img.name_loc != NONE
            || img.preamble != NONE
            || img.files.is_empty()
        {
            return Err("declaration image: not a precompiled header's image".to_owned());
        }
        let bases = validate(&img.files, &img.slocs)
            .map_err(|_| "declaration image: bad location records".to_owned())?;
        Ok(PchDecls { img, bases })
    }

    /// The buffers the image's locations are in (index 0: the header):
    /// the driver adds them to the source manager, in this order.
    #[must_use]
    pub fn files(&self) -> &[FileRec] {
        &self.img.files
    }

    /// The declaration records (parameters not counted).
    #[must_use]
    pub fn decls(&self) -> usize {
        self.img.decls.len()
    }
}

impl Writer<'_, '_, '_> {
    /// `d` is the header's own declaration: in a real file, not a
    /// predefined buffer (the consumer's sema declares those itself).
    pub(super) fn pch_own(&self, d: DeclId) -> bool {
        let sm = self.s.diags.sm();
        let n = self.s.ast.decls.get(d);
        sm.offset_of(sm.expansion_loc(n.loc()))
            .is_some_and(|(f, _)| !crate::bmi_ast::unstable_file(sm.file_name(f)))
    }
}

impl Sema<'_, '_> {
    /// Sema state the header leaves behind that the image does not carry
    /// and a later declaration of the consumer depends on: a `#pragma
    /// weak` naming nothing declared yet (it makes the consumer's
    /// definition weak), or a `#pragma redefine_extname` likewise (it
    /// renames the consumer's declaration). The image of such a header would differ from
    /// replay, so it is not written. The header's block-scope `extern`
    /// and function declarations ([`Sema::local_externs`]) are carried
    /// (WP-1293, `localexterns.rs`).
    fn pch_uncarried_state(&self) -> Option<&'static str> {
        if !self.weak_pending.is_empty() {
            return Some("a #pragma weak naming an undeclared identifier");
        }
        if !self.extname_pending.is_empty() {
            return Some("a #pragma redefine_extname naming an undeclared identifier");
        }
        None
    }

    /// The declaration image of the precompiled header whose main file is
    /// `main` (`-emit-pch`; spec/driver.md "Precompiled headers"): the
    /// section bytes, or why the header has none.
    #[must_use]
    pub fn take_pch_image(&self, main: FileId) -> ModuleImage {
        let none = |why| ModuleImage { bytes: Vec::new(), decls: 0, fallback: Some(why) };
        if self.ast.lang_c {
            return none("a C header");
        }
        if let Some(why) = self.pch_uncarried_state() {
            return none(why);
        }
        let mut w = match Writer::new(self, main, false) {
            Ok(w) => w,
            Err(why) => return none(why),
        };
        w.pch = true;
        (w.img.kw, w.img.name_loc) = (NONE, NONE);
        if let Err(why) = w.collect() {
            return none(why);
        }
        if let Some(notes) = &self.bmi.pch_ext_decls {
            w.dedup_section(notes);
        }
        (w.img.files, w.img.slocs) = w.lt.into_inner().finish();
        w.img.gated = gated_names(&w.img);
        let decls = w.img.decls.len();
        let bytes = encode(&w.img);
        if PchDecls::decode(&bytes).is_err() {
            return none("an internal image inconsistency");
        }
        ModuleImage { bytes, decls, fallback: None }
    }

    /// `-fkiln-bmi-image-census` with `-emit-pch`: the census of the
    /// header's top-level declarations ([`Sema::image_census`]).
    #[must_use]
    pub fn pch_image_census(&self, main: FileId) -> ImageCensus {
        let mut out = ImageCensus {
            module: self.diags.sm().file_name(main).to_owned(),
            ..Default::default()
        };
        let mut w = match Writer::new(self, main, true) {
            Ok(w) => w,
            Err(why) => {
                out.unit = Some(why);
                return out;
            }
        };
        w.pch = true;
        if let Err(why) = w.collect() {
            out.unit = Some(why);
        }
        let c: Census = w.census.take().unwrap_or_default();
        out.top_level = c.seen.len();
        out.rows = c
            .rows
            .into_iter()
            .map(|(reason, count, example)| super::CensusRow { reason, count, example })
            .collect();
        out.rows.sort_by_key(|r| std::cmp::Reverse(r.count));
        out.all = c.all;
        out.attrs = c.attrs.into_iter().collect();
        out.attrs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out
    }

    /// Builds the precompiled header's declarations into the translation
    /// unit, whose source manager holds the image's file table as `files`
    /// ([`PchDecls::files`], same order), the header entered from `root`
    /// (the predefines line replay includes it from). Call before the
    /// unit's first declaration.
    ///
    /// # Errors
    /// Why the image cannot be built here (nothing is built then).
    pub fn load_pch_image(
        &mut self,
        p: &PchDecls,
        files: &[FileId],
        root: Loc,
    ) -> Result<(), String> {
        let img = &p.img;
        let bad = |why: &str| Err(format!("declaration image: {why}"));
        let err = |why: &str| format!("declaration image: {why}");
        if files.len() != img.files.len() {
            return bad("a file table the consumer did not load");
        }
        if !self.image_builtins_ok(img) {
            return bad("a builtin function the consumer does not declare");
        }
        let off_ok = |o: u32| Locs::in_space(&p.bases, &img.files, &img.slocs, o);
        if !img.decls.iter().all(|d| off_ok(d.loc)) {
            return bad("an offset past the end of the header's source");
        }
        let m = Locs::build_rooted(self.diags.sm(), &img.files, &img.slocs, &p.bases, files, root)
            .map_err(err)?;
        self.build_image(img, &m);
        Ok(())
    }
}
