use acuity_schema::Placeholder;
use ts_rs::{Config, TS};

fn main() {
    // Output dir is supplied as argv[1]; defaults to dist/ inside the crate.
    // The cross-repo destination (e.g. ../cue-plugins/src) lives in the
    // invocation, never hardcoded here. This keeps the binary sandbox-ready
    // for a future Nix derivation (caller passes $out).
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "dist".to_string());

    std::fs::create_dir_all(&out_dir).expect("failed to create output directory");

    let cfg = Config::new().with_out_dir(&out_dir);
    Placeholder::export_all(&cfg).expect("ts-rs export failed");

    println!("wrote {}/types.ts", out_dir);
}
