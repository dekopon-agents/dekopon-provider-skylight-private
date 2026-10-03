//! Pure CLI parsing: no HTTP or piped data is read before broker authorization.
use clap::{Parser, Subcommand};
use dekopon_provider_sdk::provider::{Capability, Proposal, Usage};

use crate::{
    Account, Categories, Events, Frames, Items, List, Lists, SkylightPrivate, Tasks, household,
};

#[derive(Debug, Parser)]
#[command(
    name = "skylight",
    about = "Unsupported private Skylight account and frame reads over broker HTTP",
    disable_version_flag = true
)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Account,
    Frames,
    Categories {
        #[arg(long)]
        frame: String,
    },
    Events {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        tz: String,
        #[arg(long)]
        include: Option<String>,
    },
    Tasks {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        after: String,
        #[arg(long)]
        before: String,
        #[arg(long)]
        include_late: Option<bool>,
        #[arg(long)]
        include_up_for_grabs: Option<bool>,
        #[arg(long)]
        filter: Option<String>,
    },
    Lists {
        #[arg(long)]
        frame: String,
    },
    ListShow {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        list: String,
    },
    ListItems {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        list: String,
    },
}

pub fn propose(args: Args) -> Result<Proposal<SkylightPrivate>, Usage> {
    let proposal = match args.command {
        Command::Account => Proposal::to::<Account>(crate::EmptyInput {}),
        Command::Frames => Proposal::to::<Frames>(crate::EmptyInput {}),
        Command::Categories { frame } => checked::<Categories>(
            household::FrameInput::new(frame),
            household::Read::Categories,
        )?,
        Command::Lists { frame } => {
            checked::<Lists>(household::FrameInput::new(frame), household::Read::Lists)?
        }
        Command::ListShow { frame, list } => checked::<List>(
            household::ListInput::new(frame, list),
            household::Read::List,
        )?,
        Command::ListItems { frame, list } => checked::<Items>(
            household::ListInput::new(frame, list),
            household::Read::Items,
        )?,
        Command::Events {
            frame,
            from,
            to,
            tz,
            include,
        } => checked::<Events>(
            household::EventsInput::new(frame, from, to, tz, include),
            household::Read::Events,
        )?,
        Command::Tasks {
            frame,
            after,
            before,
            include_late,
            include_up_for_grabs,
            filter,
        } => checked::<Tasks>(
            household::TasksInput::new(
                frame,
                after,
                before,
                include_late,
                include_up_for_grabs,
                filter,
            ),
            household::Read::Tasks,
        )?,
    };
    Ok(proposal)
}

fn checked<C: Capability<Provider = SkylightPrivate>>(
    input: C::Input,
    read: household::Read,
) -> Result<Proposal<SkylightPrivate>, Usage> {
    let value =
        serde_json::to_value(&input).map_err(|_| Usage::new("invalid Skylight argument"))?;
    read.uri(&value)
        .map_err(|_| Usage::new("invalid Skylight argument"))?;
    Ok(Proposal::to::<C>(input))
}
