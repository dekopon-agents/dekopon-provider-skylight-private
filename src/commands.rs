//! The `skylight` command word, parsed by hand.
//!
//! Both capabilities take exactly `{}`, so the whole grammar is one bare verb. A parser crate would
//! bring a dependency closure, a license review, and component bytes to a private-data provider for
//! two words without a flag between them, so argv is matched as a slice.
//!
//! `skylight account` and `skylight frames` are *proposals*: the shell authorizes each on the path a
//! direct invocation takes — constraint-set lookup, Cedar, then credential injection inside the
//! broker's HTTP engine — and the input is the only one `invoke` accepts. `--help`, `-h`, and
//! `help` render the help page on stdout at status 0. Every other argv — no verb, an unknown verb,
//! an extra argument, any flag — renders one fixed usage error on stderr at status 2. Neither page
//! quotes argv, so caller text never comes back through the word, and piped input never reaches a
//! proposal.

use dekopon_provider_sdk::CommandRun;
use serde_json::json;

use crate::{ACCOUNT_CAPABILITY, FRAMES_CAPABILITY};

/// Exit status of the help page.
const HELP_STATUS: u8 = 0;
/// Exit status of a usage error, as a command-line program reports one.
const USAGE_STATUS: u8 = 2;

const HELP: &str = "\
Unsupported private Skylight account and frame reads over broker HTTP

Usage: skylight <COMMAND>

Commands:
  account  Read the bearer-selected account identifier (skylight.private.account.read)
  frames   List visible frame identifiers and optional names (skylight.private.frames.list)
  help     Print this help

Options:
  -h, --help  Print this help

Neither command takes arguments or flags.
";

const USAGE_ERROR: &str = "\
error: expected exactly one command: account or frames

Usage: skylight <COMMAND>

For more information, try 'skylight --help'.
";

/// Runs one `skylight` argv: the arguments after the command word.
pub(crate) fn run(argv: &[String], _stdin: Option<&str>) -> CommandRun {
    let [verb] = argv else {
        return CommandRun::rendered_error(USAGE_ERROR, USAGE_STATUS);
    };
    match verb.as_str() {
        "account" => proposal(ACCOUNT_CAPABILITY),
        "frames" => proposal(FRAMES_CAPABILITY),
        "--help" | "-h" | "help" => CommandRun::rendered(HELP, HELP_STATUS),
        _ => CommandRun::rendered_error(USAGE_ERROR, USAGE_STATUS),
    }
}

fn proposal(capability: &str) -> CommandRun {
    CommandRun::proposal(
        capability.parse().expect("static capability ID is valid"),
        json!({}),
    )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use dekopon_provider_http::Response;
    use dekopon_provider_sdk::{CommandInvocation, CommandRun, Provider};
    use serde_json::json;

    use super::run;
    use crate::{ACCOUNT_CAPABILITY, FRAMES_CAPABILITY, SkylightPrivate, invoke_with};

    // Copies rather than the constants, so an edit to either page is a diff in two places.
    const EXPECTED_HELP: &str = "\
Unsupported private Skylight account and frame reads over broker HTTP

Usage: skylight <COMMAND>

Commands:
  account  Read the bearer-selected account identifier (skylight.private.account.read)
  frames   List visible frame identifiers and optional names (skylight.private.frames.list)
  help     Print this help

Options:
  -h, --help  Print this help

Neither command takes arguments or flags.
";

    const EXPECTED_USAGE_ERROR: &str = "\
error: expected exactly one command: account or frames

Usage: skylight <COMMAND>

For more information, try 'skylight --help'.
";

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    fn proposal(words: &[&str], stdin: Option<&str>) -> CommandInvocation {
        match run(&argv(words), stdin) {
            CommandRun::Proposal(invocation) => invocation,
            other => panic!("expected a proposal for {words:?}, got {other:?}"),
        }
    }

    #[test]
    fn help_is_byte_pinned_on_stdout_at_status_zero() {
        for words in [&["--help"][..], &["-h"][..], &["help"][..]] {
            for stdin in [None, Some("piped-sentinel")] {
                assert_eq!(
                    run(&argv(words), stdin),
                    CommandRun::Rendered {
                        stdout: EXPECTED_HELP.to_owned(),
                        stderr: String::new(),
                        status: 0,
                    },
                    "{words:?}"
                );
            }
        }
    }

    /// No verb, an unknown verb, an extra argument after either verb, and any flag all render the
    /// same bytes, so nothing the caller typed comes back.
    #[test]
    fn every_other_argv_is_one_byte_pinned_usage_error_at_status_two() {
        let usage_error = CommandRun::Rendered {
            stdout: String::new(),
            stderr: EXPECTED_USAGE_ERROR.to_owned(),
            status: 2,
        };
        for words in [
            &[][..],
            &[""][..],
            &["caller-controlled-sentinel"][..],
            &["Account"][..],
            &["FRAMES"][..],
            &["--version"][..],
            &["-V"][..],
            &["--"][..],
            &["help", "account"][..],
            &["--help", "frames"][..],
            &["account", "caller-controlled-sentinel"][..],
            &["account", "--help"][..],
            &["account", "--id", "caller-controlled-sentinel"][..],
            &["account", "frames"][..],
            &["frames", "caller-controlled-sentinel"][..],
            &["frames", "-h"][..],
            &["frames", "--limit=5"][..],
            &["frames", "frames"][..],
        ] {
            for stdin in [None, Some("piped-sentinel")] {
                assert_eq!(run(&argv(words), stdin), usage_error, "{words:?}");
            }
        }
    }

    /// Both inputs are exactly `{}`: a verb's proposal has no keys at all, whatever was piped.
    #[test]
    fn each_verb_proposes_its_capability_with_no_input_keys() {
        for (verb, capability) in [
            ("account", ACCOUNT_CAPABILITY),
            ("frames", FRAMES_CAPABILITY),
        ] {
            for stdin in [None, Some("piped-sentinel"), Some(r#"{"endpoint":"x"}"#)] {
                let invocation = proposal(&[verb], stdin);
                assert_eq!(invocation.capability.as_str(), capability, "{verb}");
                assert_eq!(invocation.input, json!({}), "{verb}");
            }
        }
    }

    /// The verbs reach every manifest capability and nothing else, in manifest order.
    #[test]
    fn the_verbs_propose_exactly_the_manifest_capabilities() {
        let declared = SkylightPrivate::manifest()
            .capabilities
            .iter()
            .map(|capability| capability.id.as_str().to_owned())
            .collect::<Vec<_>>();
        let proposed = ["account", "frames"]
            .map(|verb| proposal(&[verb], None).capability.as_str().to_owned());
        assert_eq!(proposed.to_vec(), declared);
    }

    /// What the word proposes is what `invoke` accepts: one fixed request, never `invalid-input`.
    #[test]
    fn every_proposal_is_an_input_invoke_accepts() {
        for (verb, body) in [
            ("account", json!({"data": {"id": "account-7"}})),
            ("frames", json!({"data": []})),
        ] {
            let invocation = proposal(&[verb], None);
            let calls = Cell::new(0);
            invoke_with(&invocation.capability, invocation.input, |_| {
                calls.set(calls.get() + 1);
                Ok(Response {
                    status: 200,
                    headers: Vec::new(),
                    body: serde_json::to_vec(&body).expect("mock body serializes"),
                })
            })
            .unwrap_or_else(|error| panic!("{verb}: proposed input was refused: {error:?}"));
            assert_eq!(calls.get(), 1, "{verb}");
        }
    }
}
