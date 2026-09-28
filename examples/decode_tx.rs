//! Decode each hex vector file as a signed tx; exits non-zero naming every file that fails.

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut failed = false;
    for path in std::env::args().skip(1) {
        let decoded = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| hex::decode(text.trim()).map_err(|e| e.to_string()))
            .and_then(|bytes| {
                proof_wire::codec::decode_tx(&bytes)
                    .map(|_| ())
                    .map_err(|e| format!("{e:?}"))
            });
        if let Err(error) = decoded {
            println!("{path}: {error}");
            failed = true;
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
