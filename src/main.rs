mod cli;
mod config;
mod desu;
mod gelbooru;
mod http;
mod language;
mod mangadex;
mod patreon;
mod shell;

use language::{tr, trf};
use std::env;
use std::process::exit;

struct ServiceDef {
    name: &'static str,
    aliases: &'static [&'static str],
}

const SERVICES: &[ServiceDef] = &[
    ServiceDef { name: "gelbooru.com",  aliases: &["gelbu", "gel", "gelbooru"] },
    ServiceDef { name: "desu.uno",      aliases: &["desu"] },
    ServiceDef { name: "mangadex.org",  aliases: &["mangadex", "md", "mdex"] },
    ServiceDef { name: "patreon.com",   aliases: &["patreon"] },
];

/// Returns the canonical service name for the given user input (full name or alias).
fn resolve_service(input: &str) -> Option<&'static str> {
    let lower = input.to_lowercase();
    SERVICES
        .iter()
        .find(|svc| lower == svc.name || svc.aliases.iter().any(|&a| a == lower))
        .map(|svc| svc.name)
}

fn print_usage() {
    eprintln!("{}", tr("Usage:"));
    eprintln!(
        "{}",
        tr("  nymphalis <service> <command> [args...]")
    );
    eprintln!();
    eprintln!("{}", tr("Services:"));
    eprintln!(
        "{}",
        tr("  gelbooru.com  (aliases: gelbu, gel, gelbooru)")
    );
    eprintln!("{}", tr("  desu.uno      (aliases: desu)"));
    eprintln!("  patreon.com   (aliases: patreon)");
    eprintln!();
    eprintln!("{}", tr("Commands (gelbooru.com):"));
    eprintln!(
        "{}",
        tr("  nymphalis gelbooru.com download <dir> <tag1> [tag2] ...")
    );
    eprintln!(
        "{}",
        tr("      Download all images for the given tags into <dir>.")
    );
    eprintln!("{}", tr("  nymphalis gelbooru.com search <keyword>"));
    eprintln!("{}", tr("      Search for tags matching <keyword>."));
    eprintln!();
    eprintln!("{}", tr("Commands (desu.uno):"));
    eprintln!(
        "{}",
        tr("  nymphalis desu.uno download <slug.id> [slug.id2] ...")
    );
    eprintln!(
        "{}",
        tr("      Download all chapters of the given manga title(s).")
    );
    eprintln!("{}", tr("  nymphalis desu.uno search <keyword>"));
    eprintln!(
        "{}",
        tr("      Search for manga. Output: Human Title | slug.id")
    );
    eprintln!();
    eprintln!("{}", tr("Global commands:"));
    eprintln!("{}", tr("  nymphalis set <variable> <value>"));
    eprintln!(
        "{}",
        trf(
            "      Save a setting into {} . Variables: user_id, api_key, jobs.",
            &[&config::config_path().display()]
        )
    );
    eprintln!();
    eprintln!("{}", tr("Examples:"));
    eprintln!("{}", tr("  nymphalis set user_id 1955543"));
    eprintln!(
        "{}",
        tr("  nymphalis set api_key c76c2060...your_key...")
    );
    eprintln!("{}", tr("  nymphalis set jobs 8"));
    eprintln!("  nymphalis set patreon_proxy socks5h://192.168.0.1:1080");
    eprintln!("{}", tr("  nymphalis gelbooru.com search aeg"));
    eprintln!(
        "{}",
        tr("  nymphalis gelbu download ./downloads cat blue_eyes")
    );
    eprintln!("{}", tr("  nymphalis desu.uno search gals"));
    eprintln!(
        "{}",
        tr("  nymphalis desu download imaizumins-house-is-a-place-for-gals-to-gather.5467")
    );
    eprintln!();
    eprintln!("Commands (patreon.com):");
    eprintln!("  nymphalis patreon login");
    eprintln!("  nymphalis patreon show");
    eprintln!("  nymphalis patreon download ./dir creator1 creator2");
}

fn main() {
    let _ = dotenvy::dotenv();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        exit(1);
    }

    // Global commands that don't require a service selector
    match args[1].as_str() {
        "set" => {
            config::run_set(&args[2..]);
            return;
        }
        "add-shell-completions" => {
            shell::install();
            return;
        }
        "-h" | "--help" => {
            print_usage();
            exit(0);
        }
        _ => {}
    }

    let service = match resolve_service(&args[1]) {
        Some(s) => s,
        None => {
            eprintln!("{}", trf("Unknown service: {}\n", &[&&args[1]]));
            print_usage();
            exit(1);
        }
    };

    if args.len() < 3 {
        eprintln!(
            "{}",
            trf("Unknown command for {}: {}\n", &[&service, &"(none)"])
        );
        print_usage();
        exit(1);
    }

    match service {
        "gelbooru.com" => gelbooru::dispatch(&args[2..]),
        "desu.uno"     => desu::dispatch(&args[2..]),
        "mangadex.org" => mangadex::dispatch(&args[2..]),
        "patreon.com"  => patreon::dispatch(&args[2..]),
        _ => unreachable!(),
    }
}
