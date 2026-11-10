use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

pub struct FileWorld {
    root: PathBuf,
    main: FileId,
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    sources: Mutex<HashMap<FileId, Source>>,
}

impl FileWorld {
    pub fn new(main_path: &Path) -> Result<Self, String> {
        let root = main_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let name = main_path
            .file_name()
            .ok_or_else(|| format!("no file name in {}", main_path.display()))?;
        let name = name
            .to_str()
            .ok_or_else(|| format!("non-UTF-8 file name: {}", main_path.display()))?;
        let vpath = VirtualPath::new(name).map_err(|e| format!("invalid main path: {e:?}"))?;
        let main = RootedPath::new(VirtualRoot::Project, vpath).intern();

        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data.to_vec())))
            .collect();
        let book = FontBook::from_fonts(&fonts);

        Ok(Self {
            root,
            main,
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
            sources: Mutex::new(HashMap::new()),
        })
    }

    pub fn compile(&self) -> Result<PagedDocument, Vec<SourceDiagnostic>> {
        let warned = typst::compile::<PagedDocument>(self);
        warned.output.map_err(|errors| errors.to_vec())
    }

    fn resolve(&self, id: FileId) -> FileResult<PathBuf> {
        if !matches!(id.root(), VirtualRoot::Project) {
            return Err(FileError::Other(Some("packages are not supported".into())));
        }
        id.vpath()
            .realize(&self.root)
            .map_err(|_| FileError::AccessDenied)
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

    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        Datetime::from_ymd(2025, 1, 1)
    }
}
