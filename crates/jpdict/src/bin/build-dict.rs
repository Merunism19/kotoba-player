//! `build-dict <jmdict.json> <accents.txt> <out.sqlite>`: compile the dictionary database.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [jmdict, accents, out] = args.as_slice() else {
        eprintln!("usage: build-dict <jmdict.json> <accents.txt> <out.sqlite>");
        return ExitCode::from(2);
    };
    match jpdict::build::build_dict(Path::new(jmdict), Path::new(accents), Path::new(out)) {
        Ok(s) => {
            println!(
                "built {out}: {} entries, {} forms, {} pitch rows, {:.1} MB in {:.1} s",
                s.entries,
                s.forms,
                s.pitch_rows,
                s.db_bytes as f64 / 1e6,
                s.seconds
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("build-dict failed: {e}");
            ExitCode::FAILURE
        }
    }
}
