//! `kobold-governance` CLI: diff two copybook layout versions and emit a drift report plus an on-disk
//! layout-compatibility verdict (JSON). Fail-closed: a non-zero exit means the verdict is a
//! `BreakingChange` -- the new copybook cannot safely read records written under the old one.
//!
//! Usage:
//!   kobold-governance diff <old.json> <new.json> [--pretty]
//!
//! Each input JSON is a [`kobold_governance::Copybook`], e.g.:
//!   {"name":"ACCT-REC","version":"v1","fields":[
//!      {"name":"ACCT-ID","level":5,"offset":0,"len":8,"kind":{"display":{"digits":8,"scale":0,"signed":false}}},
//!      {"name":"ACCT-BAL","level":5,"offset":8,"len":5,"kind":{"packed":{"digits":9,"scale":2,"signed":true}}}]}
#![forbid(unsafe_code)]

use kobold_governance::{compatibility, diff, Compatibility, Copybook, DriftReport};
use serde::Serialize;
use std::process::ExitCode;

/// The CLI output: the drift report and the collapsed verdict, serialized together.
#[derive(Serialize)]
struct Output {
    report: DriftReport,
    verdict: Compatibility,
}

fn load(path: &str) -> Result<Copybook, String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    serde_json::from_str(&src).map_err(|e| format!("invalid copybook JSON in {path}: {e}"))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 || args[1] != "diff" {
        eprintln!("usage: kobold-governance diff <old.json> <new.json> [--pretty]");
        return ExitCode::from(2);
    }

    let mut positionals: Vec<String> = Vec::new();
    let mut pretty = false;
    for a in &args[2..] {
        match a.as_str() {
            "--pretty" => pretty = true,
            other => positionals.push(other.to_string()),
        }
    }

    let [old_path, new_path] = positionals.as_slice() else {
        eprintln!("error: need exactly <old.json> and <new.json>");
        return ExitCode::from(2);
    };

    let old = match load(old_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let new = match load(new_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    let report = diff(&old, &new);
    let verdict = compatibility(&report);
    let out = Output { report, verdict };

    let json = if pretty {
        serde_json::to_string_pretty(&out)
    } else {
        serde_json::to_string(&out)
    };
    match json {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("error: serialize output: {e}");
            return ExitCode::from(2);
        }
    }

    match verdict {
        Compatibility::Identical | Compatibility::CompatibleExtension => ExitCode::SUCCESS,
        Compatibility::BreakingChange => ExitCode::FAILURE,
    }
}
