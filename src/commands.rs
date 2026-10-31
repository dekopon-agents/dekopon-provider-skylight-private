//! Pure CLI parsing: no HTTP or piped data is read before broker authorization.
use clap::{Parser, Subcommand};
use dekopon_provider_sdk::provider::{Capability, Proposal, Usage};

use crate::{
    Account, Categories, EventCreate, EventDelete, EventUpdate, Events, Frames, Items, List, Lists,
    SkylightPrivate, Tasks, household,
};

#[derive(Debug, Parser)]
#[command(
    name = "skylight",
    about = "Unsupported private Skylight household reads and calendar event writes over broker HTTP",
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
    EventAdd {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        starts_at: Option<String>,
        #[arg(long)]
        ends_at: Option<String>,
        #[arg(long)]
        all_day: Option<bool>,
        #[arg(long)]
        tz: Option<String>,
        /// Repeat once per category identifier.
        #[arg(long)]
        category: Vec<String>,
        /// Repeat once per recurrence rule string.
        #[arg(long)]
        rrule: Vec<String>,
    },
    EventUpdate {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        event: String,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long)]
        starts_at: Option<String>,
        #[arg(long)]
        ends_at: Option<String>,
        #[arg(long)]
        all_day: Option<bool>,
        #[arg(long)]
        tz: Option<String>,
    },
    EventDelete {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        event: String,
    },
}

pub fn propose(args: Args) -> Result<Proposal<SkylightPrivate>, Usage> {
    let proposal = match args.command {
        Command::Account => Proposal::to::<Account>(crate::EmptyInput {}),
        Command::Frames => Proposal::to::<Frames>(crate::EmptyInput {}),
        Command::Categories { frame } => {
            checked::<Categories>(household::FrameInput::new(frame), |input| {
                household::Read::Categories.uri(input)
            })?
        }
        Command::Lists { frame } => checked::<Lists>(household::FrameInput::new(frame), |input| {
            household::Read::Lists.uri(input)
        })?,
        Command::ListShow { frame, list } => {
            checked::<List>(household::ListInput::new(frame, list), |input| {
                household::Read::List.uri(input)
            })?
        }
        Command::ListItems { frame, list } => {
            checked::<Items>(household::ListInput::new(frame, list), |input| {
                household::Read::Items.uri(input)
            })?
        }
        Command::Events {
            frame,
            from,
            to,
            tz,
            include,
        } => checked::<Events>(
            household::EventsInput::new(frame, from, to, tz, include),
            |input| household::Read::Events.uri(input),
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
            |input| household::Read::Tasks.uri(input),
        )?,
        Command::EventAdd {
            frame,
            summary,
            starts_at,
            ends_at,
            all_day,
            tz,
            category,
            rrule,
        } => checked::<EventCreate>(
            household::EventCreateInput::new(
                frame, summary, starts_at, ends_at, all_day, tz, category, rrule,
            ),
            |input| household::Mutation::EventCreate.uri(input),
        )?,
        Command::EventUpdate {
            frame,
            event,
            summary,
            starts_at,
            ends_at,
            all_day,
            tz,
        } => checked::<EventUpdate>(
            household::EventUpdateInput::new(
                frame, event, summary, starts_at, ends_at, all_day, tz,
            ),
            |input| household::Mutation::EventUpdate.uri(input),
        )?,
        Command::EventDelete { frame, event } => {
            checked::<EventDelete>(household::EventDeleteInput::new(frame, event), |input| {
                household::Mutation::EventDelete.uri(input)
            })?
        }
    };
    Ok(proposal)
}

/// Refuses argv the capability itself would refuse, so a proposal never carries an invalid input.
/// `validate` is the capability's own full-input check (a write's route check validates its body).
fn checked<C: Capability<Provider = SkylightPrivate>>(
    input: C::Input,
    validate: impl FnOnce(&serde_json::Value) -> Result<String, crate::ProviderError>,
) -> Result<Proposal<SkylightPrivate>, Usage> {
    let value =
        serde_json::to_value(&input).map_err(|_| Usage::new("invalid Skylight argument"))?;
    validate(&value).map_err(|_| Usage::new("invalid Skylight argument"))?;
    Ok(Proposal::to::<C>(input))
}
