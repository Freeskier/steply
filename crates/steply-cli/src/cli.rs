use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;

use clap::builder::PossibleValuesParser;
use clap::{Arg, ArgAction, ArgMatches, Command, error::ErrorKind};
use steply_core::config::{ConfigDocs, FieldDoc, WidgetDoc, schema_docs};
use steply_runtime::{RenderJsonRequest, StartOptions};

use crate::flow::FlowInvocation;
use crate::prompt::PromptInvocation;

pub enum Invocation {
    Run(StartOptions),
    Prompt(PromptInvocation),
    Export(ExportInvocation),
    Flow(FlowInvocation),
}

pub struct ExportInvocation {
    pub kind: ExportKind,
    pub out_path: PathBuf,
}

pub enum ExportKind {
    Schema,
    Docs,
}

pub fn parse_invocation() -> Result<Invocation, clap::Error> {
    parse_invocation_from(std::env::args_os())
}

fn parse_invocation_from(
    args: impl IntoIterator<Item = OsString>,
) -> Result<Invocation, clap::Error> {
    let docs = schema_docs().map_err(|err| {
        clap::Error::raw(
            ErrorKind::Io,
            format!("failed to load widget docs for CLI generation: {err}"),
        )
    })?;
    let docs_by_command = build_docs_lookup(&docs);
    let matches = build_cli(&docs).try_get_matches_from(args)?;

    if let Some((name, sub_matches)) = matches.subcommand() {
        return match name {
            "run" => Ok(Invocation::Run(parse_run_options(sub_matches)?)),
            "export-schema" => Ok(Invocation::Export(parse_export_invocation(
                ExportKind::Schema,
                sub_matches,
            )?)),
            "export-docs" => Ok(Invocation::Export(parse_export_invocation(
                ExportKind::Docs,
                sub_matches,
            )?)),
            "flow" => Ok(Invocation::Flow(parse_flow_invocation(sub_matches)?)),
            other => {
                let Some(doc) = docs_by_command.get(other).cloned() else {
                    return Err(clap::Error::raw(
                        ErrorKind::UnknownArgument,
                        format!("unknown command: {other}"),
                    ));
                };
                Ok(Invocation::Prompt(parse_prompt_invocation(
                    doc,
                    sub_matches,
                )?))
            }
        };
    }

    Ok(Invocation::Run(parse_run_options(&matches)?))
}

fn build_cli(docs: &ConfigDocs) -> Command {
    let mut command = add_run_args(
        Command::new("steply")
            .about("Terminal prompt renderer and YAML-driven wizard runtime.")
            .subcommand_required(false)
            .arg_required_else_help(false)
            .disable_help_subcommand(true),
    )
    .subcommand(add_run_args(
        Command::new("run").about("Run a full Steply flow from YAML or render JSON preview."),
    ))
    .subcommand(build_export_command(
        "export-schema",
        "Export the generated JSON Schema for the YAML config format.",
    ))
    .subcommand(build_export_command(
        "export-docs",
        "Export the generated docs JSON consumed by the web documentation.",
    ))
    .subcommand(build_flow_command());

    let mut widgets = docs.widgets.clone();
    widgets.sort_by(|a, b| a.widget_type.cmp(b.widget_type));
    for doc in widgets {
        command = command.subcommand(build_widget_command(&doc));
    }

    command
}

fn build_flow_command() -> Command {
    Command::new("flow")
        .about("Create, build and run draft flows from shell scripts.")
        .subcommand_required(true)
        .subcommand(
            Command::new("create")
                .visible_alias("start")
                .about("Create a new draft flow and print its id.")
                .arg(
                    Arg::new("decorate")
                        .long("decorate")
                        .action(ArgAction::SetTrue)
                        .help("Render the flow with normal Steply chrome when running it."),
                ),
        )
        .subcommand(
            Command::new("step")
                .about("Create or select the current step for a draft flow.")
                .arg(
                    Arg::new("flow_id")
                        .required(true)
                        .value_name("FLOW_ID")
                        .help("Draft flow id returned by `steply flow create`."),
                )
                .arg(
                    Arg::new("title")
                        .long("title")
                        .required(true)
                        .value_name("TITLE")
                        .help("Title used for the step when the draft flow is run."),
                )
                .arg(
                    Arg::new("id")
                        .long("id")
                        .value_name("STEP_ID")
                        .help("Optional explicit step id. If omitted, one is generated."),
                ),
        )
        .subcommand(
            Command::new("run")
                .about("Run a previously built draft flow.")
                .arg(
                    Arg::new("flow_id")
                        .required(true)
                        .value_name("FLOW_ID")
                        .help("Draft flow id to run."),
                ),
        )
        .subcommand(
            Command::new("export")
                .about("Export a draft flow as runnable YAML.")
                .arg(
                    Arg::new("flow_id")
                        .required(true)
                        .value_name("FLOW_ID")
                        .help("Draft flow id to export."),
                )
                .arg(
                    Arg::new("out")
                        .long("out")
                        .required(true)
                        .value_name("PATH")
                        .help("Destination YAML file."),
                ),
        )
        .subcommand(
            Command::new("drop")
                .visible_alias("end")
                .about("Delete a draft flow from local storage.")
                .arg(
                    Arg::new("flow_id")
                        .required(true)
                        .value_name("FLOW_ID")
                        .help("Draft flow id to delete."),
                ),
        )
}

fn add_run_args(command: Command) -> Command {
    command
        .arg(
            Arg::new("config")
                .long("config")
                .value_name("PATH")
                .help("Path to YAML config. Use '-' to read YAML from stdin."),
        )
        .arg(
            Arg::new("render_json")
                .long("render-json")
                .action(ArgAction::SetTrue)
                .help("Print preview render JSON instead of running the interactive flow."),
        )
        .arg(
            Arg::new("render_scope")
                .long("render-scope")
                .value_name("SCOPE")
                .help("Preview scope: app, step or widget."),
        )
        .arg(
            Arg::new("render_step_id")
                .long("render-step-id")
                .value_name("STEP_ID")
                .help("Target step id for render preview."),
        )
        .arg(
            Arg::new("render_widget_id")
                .long("render-widget-id")
                .value_name("WIDGET_ID")
                .help("Target widget id for render preview."),
        )
        .arg(
            Arg::new("render_active_step_id")
                .long("render-active-step-id")
                .value_name("STEP_ID")
                .help("Active step id used while rendering preview JSON."),
        )
        .arg(
            Arg::new("render_width")
                .long("render-width")
                .value_name("WIDTH")
                .help("Render preview width."),
        )
        .arg(
            Arg::new("render_height")
                .long("render-height")
                .value_name("HEIGHT")
                .help("Render preview height."),
        )
}

fn build_widget_command(doc: &WidgetDoc) -> Command {
    let command_name = command_name(doc.widget_type);
    let mut command = Command::new(command_name.clone())
        .about(doc.short_description)
        .long_about(widget_long_about(doc))
        .arg(
            Arg::new("flow")
                .long("flow")
                .value_name("FLOW_ID")
                .help("Append this widget to a draft flow instead of running it immediately."),
        );

    if command_name != doc.widget_type {
        command = command.visible_alias(doc.widget_type);
    }

    for field in &doc.fields {
        command = command.arg(build_widget_arg(field));
    }

    command
}

fn build_export_command(name: &'static str, about: &'static str) -> Command {
    Command::new(name).about(about).arg(
        Arg::new("out")
            .long("out")
            .value_name("PATH")
            .required(true)
            .help("Destination path for the generated JSON file."),
    )
}

fn build_widget_arg(field: &FieldDoc) -> Arg {
    let mut arg = Arg::new(field.name.clone())
        .long(flag_name(field.name.as_str()))
        .help(field.short_description.clone())
        .long_help(field_long_help(field))
        .value_name(field_value_name(field))
        .required(field.required && !is_auto_default_field(field.name.as_str()));

    if field.name == "submit_target" {
        arg = arg.visible_alias("target");
    }

    if !field.allowed_values.is_empty() && !is_list_type(field.type_name.as_str()) {
        arg = arg.value_parser(PossibleValuesParser::new(field.allowed_values.clone()));
    }

    if is_list_type(field.type_name.as_str()) {
        arg.action(ArgAction::Append)
    } else {
        arg.action(ArgAction::Set)
    }
}

fn parse_run_options(matches: &ArgMatches) -> Result<StartOptions, clap::Error> {
    let config_path = matches.get_one::<String>("config").map(PathBuf::from);
    let render_json = if matches.get_flag("render_json") {
        Some(
            RenderJsonRequest::from_named_parts(
                matches.get_one::<String>("render_scope").cloned(),
                matches.get_one::<String>("render_step_id").cloned(),
                matches.get_one::<String>("render_widget_id").cloned(),
                matches.get_one::<String>("render_active_step_id").cloned(),
                parse_optional_u16(matches.get_one::<String>("render_width"), "--render-width")?,
                parse_optional_u16(
                    matches.get_one::<String>("render_height"),
                    "--render-height",
                )?,
            )
            .map_err(|err| clap::Error::raw(ErrorKind::ValueValidation, err))?,
        )
    } else {
        None
    };

    Ok(StartOptions {
        config_path,
        render_json,
    })
}

fn parse_prompt_invocation(
    doc: WidgetDoc,
    matches: &ArgMatches,
) -> Result<PromptInvocation, clap::Error> {
    let mut values = HashMap::new();
    for field in &doc.fields {
        if let Some(raw_values) = matches.get_many::<String>(field.name.as_str()) {
            values.insert(field.name.clone(), raw_values.cloned().collect::<Vec<_>>());
        }
    }

    Ok(PromptInvocation {
        doc,
        values,
        flow_id: matches.get_one::<String>("flow").cloned(),
    })
}

fn parse_flow_invocation(matches: &ArgMatches) -> Result<FlowInvocation, clap::Error> {
    match matches.subcommand() {
        Some(("create", sub_matches)) => Ok(FlowInvocation::Create {
            decorate: sub_matches.get_flag("decorate"),
        }),
        Some(("step", sub_matches)) => Ok(FlowInvocation::Step {
            flow_id: required_string(sub_matches, "flow_id")?,
            title: required_string(sub_matches, "title")?,
            step_id: sub_matches.get_one::<String>("id").cloned(),
        }),
        Some(("run", sub_matches)) => Ok(FlowInvocation::Run {
            flow_id: required_string(sub_matches, "flow_id")?,
        }),
        Some(("export", sub_matches)) => Ok(FlowInvocation::Export {
            flow_id: required_string(sub_matches, "flow_id")?,
            out_path: PathBuf::from(required_string(sub_matches, "out")?),
        }),
        Some(("drop", sub_matches)) => Ok(FlowInvocation::Drop {
            flow_id: required_string(sub_matches, "flow_id")?,
        }),
        _ => Err(clap::Error::raw(
            ErrorKind::MissingSubcommand,
            "missing `steply flow` subcommand",
        )),
    }
}

fn parse_export_invocation(
    kind: ExportKind,
    matches: &ArgMatches,
) -> Result<ExportInvocation, clap::Error> {
    let out_path = matches
        .get_one::<String>("out")
        .map(PathBuf::from)
        .ok_or_else(|| clap::Error::raw(ErrorKind::MissingRequiredArgument, "missing --out"))?;

    Ok(ExportInvocation { kind, out_path })
}

fn parse_optional_u16(value: Option<&String>, arg_name: &str) -> Result<Option<u16>, clap::Error> {
    value
        .map(|raw| {
            raw.parse::<u16>().map_err(|_| {
                clap::Error::raw(
                    ErrorKind::ValueValidation,
                    format!("invalid value for {arg_name}: {raw}"),
                )
            })
        })
        .transpose()
}

fn build_docs_lookup(docs: &ConfigDocs) -> HashMap<String, WidgetDoc> {
    docs.widgets
        .iter()
        .cloned()
        .flat_map(|doc| {
            let command = command_name(doc.widget_type);
            let alias = doc.widget_type.to_string();
            [(command, doc.clone()), (alias, doc)]
        })
        .collect()
}

fn required_string(matches: &ArgMatches, name: &str) -> Result<String, clap::Error> {
    matches.get_one::<String>(name).cloned().ok_or_else(|| {
        clap::Error::raw(
            ErrorKind::MissingRequiredArgument,
            format!("missing {name}"),
        )
    })
}

fn command_name(widget_type: &str) -> String {
    widget_type.replace('_', "-")
}

fn flag_name(field_name: &str) -> String {
    field_name.replace('_', "-")
}

fn field_value_name(field: &FieldDoc) -> &'static str {
    if is_list_type(field.type_name.as_str()) {
        "ITEM"
    } else {
        "VALUE"
    }
}

fn field_long_help(field: &FieldDoc) -> String {
    let mut lines = Vec::new();
    if !field.short_description.is_empty() {
        lines.push(field.short_description.clone());
    }
    if let Some(long) = &field.long_description
        && !long.is_empty()
    {
        lines.push(long.clone());
    }

    lines.push(format!("YAML field: `{}`", field.name));
    if field.name == "submit_target" {
        lines.push("CLI alias: `--target`".to_string());
    }
    lines.push(format!("Type: `{}`", field.type_name));
    lines.push(format!(
        "Required: {}",
        if is_auto_default_field(field.name.as_str()) && field.required {
            "yes (auto-filled in prompt mode)"
        } else if field.required {
            "yes"
        } else {
            "no"
        }
    ));
    if let Some(default) = &field.default {
        lines.push(format!("Default: `{default}`"));
    }
    if !field.allowed_values.is_empty() {
        lines.push(format!(
            "Allowed values: {}",
            field
                .allowed_values
                .iter()
                .map(|value| format!("`{value}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if is_list_type(field.type_name.as_str()) {
        lines.push(
            "Repeat this flag multiple times or pass a full YAML/JSON list in one value."
                .to_string(),
        );
    } else if needs_yaml_fragment(field.type_name.as_str()) {
        lines.push("Pass a YAML/JSON fragment for complex values.".to_string());
    }

    lines.join("\n")
}

fn widget_long_about(doc: &WidgetDoc) -> String {
    let mut sections = vec![doc.long_description.to_string()];
    if !doc.static_hints.is_empty() {
        sections.push(format!(
            "Static hints: {}",
            doc.static_hints
                .iter()
                .map(|hint| format!("{} {}", hint.key, hint.label))
                .collect::<Vec<_>>()
                .join("  •  ")
        ));
    }
    sections.push(format!("Example YAML:\n{}", doc.example_yaml));
    sections.join("\n")
}

fn is_auto_default_field(field_name: &str) -> bool {
    matches!(field_name, "id" | "label")
}

fn is_list_type(type_name: &str) -> bool {
    type_name.starts_with("list<")
}

fn needs_yaml_fragment(type_name: &str) -> bool {
    type_name == "object" || type_name.contains(" | ") || is_list_type(type_name)
}
