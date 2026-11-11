use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use typst::diag::{FileError, FileResult, PackageError, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::files::FsRoot;
use typst_kit::packages::FsPackages;
use typst_layout::PagedDocument;

use crate::project::Project;

pub struct FileWorld {
    root: PathBuf,
    main: FileId,
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    /// Package directories in the order the Typst CLI searches them: data, then cache.
    /// Packages are never downloaded.
    packages: Vec<FsPackages>,
    /// The date `datetime.today()` returns: the system clock, or a fixed date for reproducible output.
    time: Time,
    sources: Mutex<HashMap<FileId, Source>>,
}

impl FileWorld {
    pub fn new(project: &Project, time: Time) -> Self {
        let main = RootedPath::new(VirtualRoot::Project, project.main.clone()).intern();

        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data.to_vec())))
            .collect();
        let book = FontBook::from_fonts(&fonts);

        Self {
            root: project.root.clone(),
            main,
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
            packages: [FsPackages::system_data(), FsPackages::system_cache()].into_iter().flatten().collect(),
            time,
            sources: Mutex::new(HashMap::new()),
        }
    }

    pub fn compile(&self) -> Result<PagedDocument, Vec<SourceDiagnostic>> {
        let warned = typst::compile::<PagedDocument>(self);
        warned.output.map_err(|errors| errors.to_vec())
    }

    fn resolve(&self, id: FileId) -> FileResult<PathBuf> {
        match id.root() {
            VirtualRoot::Project => id
                .vpath()
                .realize(&self.root)
                .map_err(|_| FileError::AccessDenied),
            VirtualRoot::Package(spec) => self.package_root(spec)?.resolve(id.vpath()),
        }
    }

    fn package_root(&self, spec: &PackageSpec) -> FileResult<FsRoot> {
        self.packages
            .iter()
            .find_map(|packages| packages.obtain(spec))
            .ok_or_else(|| FileError::Package(PackageError::NotFound(spec.clone())))
    }
}

impl World for FileWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if let Some(source) = self.sources.lock().unwrap().get(&id) {
            return Ok(source.clone());
        }
        let path = self.resolve(id)?;
        let text = fs::read_to_string(&path).map_err(|e| FileError::from_io(e, &path))?;
        let source = Source::new(id, text);
        self.sources.lock().unwrap().insert(id, source.clone());
        Ok(source)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = self.resolve(id)?;
        let data = fs::read(&path).map_err(|e| FileError::from_io(e, &path))?;
        Ok(Bytes::new(data))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        self.time.today(offset)
    }
}
