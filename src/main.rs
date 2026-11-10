mod extract;
mod trace;
mod world;

use std::path::PathBuf;
use std::process::ExitCode;

use world::FileWorld;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut input: Option<PathBuf> = None;
    let mut dump_trace = false;
    for arg in args.by_ref() {
        match arg.as_str() {
            "--trace" => dump_trace = true,
            _ => input = Some(PathBuf::from(arg)),
        }
    }
    let Some(input) = input else {
        eprintln!("usage: typst-prose <input.typ> [--trace]");
        return ExitCode::from(2);
    };

    let world = match FileWorld::new(&input) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let doc = match world.compile() {
        Ok(doc) => doc,
        Err(errors) => {
            for e in errors {
                eprintln!("error: {}", e.message);
            }
            return ExitCode::from(1);
        }
    };

    if dump_trace {
        let mut tracer = trace::Tracer::new(&world);
        tracer.document(&doc);
        print!("{}", tracer.out);
        return ExitCode::SUCCESS;
    }

    let extraction = extract::extract(&doc);
    for text in &extraction.untagged {
        eprintln!("untagged text: {text:?}");
    }
    if extraction.dropped_artifacts > 0 {
        eprintln!("suppressed text runs (artifact or footnote reference): {}", extraction.dropped_artifacts);
    }
    if extraction.unmatched_tags > 0 {
        eprintln!("unmatched tags: {}", extraction.unmatched_tags);
    }
    print!("{}", extraction.text());
    ExitCode::SUCCESS
}
