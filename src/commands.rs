//! Pure CLI parsing: no HTTP or piped data is read before broker authorization.
use clap::{Parser, Subcommand};
use dekopon_provider_sdk::provider::{Capability, Proposal, Usage};

use crate::{
    Account, Categories, CategoryCreate, CategoryDelete, CategoryFindOrCreate, CategoryUpdate,
    EventCreate, EventDelete, EventUpdate, Events, Frames, ItemComplete, ItemCreate, ItemDelete,
    ItemUpdate, Items, List, ListCreate, ListDelete, ListUpdate, Lists, SkylightPrivate, Tasks,
    household,
};

#[derive(Debug, Parser)]
#[command(
    name = "skylight",
    about = "Unsupported private Skylight household reads and calendar event, list, list item and category writes over broker HTTP",
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
    ListAdd {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        list: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        section: Option<String>,
    },
    ListItemUpdate {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        list: String,
        #[arg(long)]
        item: String,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        position: Option<u64>,
    },
    ListItemComplete {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        list: String,
        #[arg(long)]
        item: String,
        #[arg(long)]
        status: Option<String>,
    },
    ListItemDelete {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        list: String,
        #[arg(long)]
        item: String,
    },
    ListCreate {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        kind: Option<String>,
    },
    ListUpdate {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        list: String,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        kind: Option<String>,
    },
    ListDelete {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        list: String,
    },
    CategoryAdd {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        color: String,
    },
    CategoryFindOrCreate {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        color: String,
    },
    CategoryUpdate {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        category: String,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        color: Option<String>,
    },
    CategoryDelete {
        #[arg(long)]
        frame: String,
        #[arg(long)]
        category: String,
        #[arg(long)]
        reassign_to: Option<String>,
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
        Command::ListAdd {
            frame,
            list,
            label,
            section,
        } => checked::<ItemCreate>(
            household::ItemCreateInput::new(frame, list, label, section),
            |input| household::Mutation::ItemCreate.uri(input),
        )?,
        Command::ListItemUpdate {
            frame,
            list,
            item,
            label,
            status,
            position,
        } => checked::<ItemUpdate>(
            household::ItemUpdateInput::new(frame, list, item, label, status, position),
            |input| household::Mutation::ItemUpdate.uri(input),
        )?,
        Command::ListItemComplete {
            frame,
            list,
            item,
            status,
        } => checked::<ItemComplete>(
            household::ItemCompleteInput::new(frame, list, item, status),
            |input| household::Mutation::ItemComplete.uri(input),
        )?,
        Command::ListItemDelete { frame, list, item } => checked::<ItemDelete>(
            household::ItemDeleteInput::new(frame, list, item),
            |input| household::Mutation::ItemDelete.uri(input),
        )?,
        Command::ListCreate {
            frame,
            label,
            color,
            kind,
        } => checked::<ListCreate>(
            household::ListCreateInput::new(frame, label, color, kind),
            |input| household::Mutation::ListCreate.uri(input),
        )?,
        Command::ListUpdate {
            frame,
            list,
            label,
            color,
            kind,
        } => checked::<ListUpdate>(
            household::ListUpdateInput::new(frame, list, label, color, kind),
            |input| household::Mutation::ListUpdate.uri(input),
        )?,
        Command::ListDelete { frame, list } => {
            checked::<ListDelete>(household::ListInput::new(frame, list), |input| {
                household::Mutation::ListDelete.uri(input)
            })?
        }
        Command::CategoryAdd {
            frame,
            label,
            color,
        } => checked::<CategoryCreate>(
            household::CategoryInput::new(frame, label, color),
            |input| household::Mutation::CategoryCreate.uri(input),
        )?,
        Command::CategoryFindOrCreate {
            frame,
            label,
            color,
        } => checked::<CategoryFindOrCreate>(
            household::CategoryInput::new(frame, label, color),
            |input| household::Mutation::CategoryFindOrCreate.uri(input),
        )?,
        Command::CategoryUpdate {
            frame,
            category,
            label,
            color,
        } => checked::<CategoryUpdate>(
            household::CategoryUpdateInput::new(frame, category, label, color),
            |input| household::Mutation::CategoryUpdate.uri(input),
        )?,
        Command::CategoryDelete {
            frame,
            category,
            reassign_to,
        } => checked::<CategoryDelete>(
            household::CategoryDeleteInput::new(frame, category, reassign_to),
            |input| household::Mutation::CategoryDelete.uri(input),
        )?,
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
