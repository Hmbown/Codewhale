use clap::{Parser, Subcommand};
use codewhale_memory::{import, protocol::ToolServer, workspace, *};
use serde::de::DeserializeOwned;
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(
    name = "cw-memory",
    version,
    about = "CodeWhale local memory: evidence, review, recall and checkpoints"
)]
struct Args {
    /// A NEW database, never the existing native index.sqlite3.
    #[arg(long)]
    db: PathBuf,
    #[arg(long, default_value = "local")]
    tenant: String,
    #[arg(long, default_value = "local")]
    user: String,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    branch: Option<String>,
    #[arg(long)]
    session: Option<String>,
    #[arg(long)]
    agent: Option<String>,
    /// Trusted working-tree root for live dependency hashes.
    #[arg(long)]
    workspace_root: Option<PathBuf>,
    /// Explicit local reviewer/admin authority. Never forward model arguments here.
    #[arg(long, conflicts_with = "read_only")]
    operator: bool,
    #[arg(long)]
    read_only: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Status,
    /// Read a scoped Context Lens snapshot without granting new permissions.
    Lens {
        #[arg(long)]
        trace: Option<String>,
        #[arg(long)]
        after: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Export metadata-only lifecycle observations, optionally as event-v1 JSONL.
    Events {
        #[arg(long, default_value_t = 0)]
        after: i64,
        #[arg(long, default_value_t = 200)]
        limit: usize,
        #[arg(long)]
        jsonl: bool,
    },
    Preferences {
        id: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        pinned: bool,
        #[arg(long)]
        suppressed: bool,
    },
    History {
        id: String,
        #[arg(long)]
        known_at: i64,
        #[arg(long)]
        valid_at: i64,
    },
    List {
        #[arg(long)]
        after: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    Propose {
        #[arg(long)]
        request: String,
        #[arg(long)]
        file: PathBuf,
    },
    Search {
        query: String,
        #[arg(long, default_value_t = 12)]
        limit: usize,
        #[arg(long)]
        include_stale: bool,
        #[arg(long)]
        embedding: Option<PathBuf>,
    },
    Get {
        id: String,
    },
    Approve {
        id: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        validation: Option<PathBuf>,
    },
    Reject {
        id: String,
        #[arg(long)]
        revision: i64,
    },
    Supersede {
        old: String,
        new: String,
        #[arg(long)]
        old_revision: i64,
        #[arg(long)]
        new_revision: i64,
        #[arg(long)]
        validation: Option<PathBuf>,
    },
    Forget {
        id: String,
        #[arg(long)]
        revision: i64,
    },
    Link {
        from: String,
        to: String,
        #[arg(long, default_value = "related")]
        relation: String,
    },
    Embed {
        id: String,
        #[arg(long)]
        content_hash: String,
        #[arg(long)]
        file: PathBuf,
    },
    Context {
        #[arg(default_value = "")]
        query: String,
        #[arg(long, default_value_t = 12000)]
        max_bytes: usize,
    },
    CheckpointSave {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        expected_revision: Option<i64>,
    },
    Resume {
        key: String,
    },
    ImportMarkdown {
        file: PathBuf,
    },
    ImportJsonl {
        file: PathBuf,
    },
    Export,
    Reindex,
    Expire,
    /// MCP stdio compatibility server, revisions 2025-06-18 and 2025-11-25.
    Serve,
}
fn read_bounded(path: &Path, max: usize) -> Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((max + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(Error::Invalid("input file exceeds size limit".into()));
    }
    String::from_utf8(bytes).map_err(|_| Error::Invalid("input must be UTF-8".into()))
}
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_str(&read_bounded(path, 64 * 1024)?)?)
}
fn print<T: serde::Serialize>(value: &T) -> Result<()> {
    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value)?;
    use std::io::Write;
    out.write_all(b"\n")?;
    Ok(())
}
fn run(args: Args) -> Result<()> {
    let global = Scope::user(args.tenant, args.user);
    let mut scopes = vec![global.clone()];
    let mut current = global;
    if let Some(w) = args.workspace {
        current = current.workspace(w);
        scopes.push(current.clone());
    }
    if let Some(b) = args.branch {
        current = current.branch(b);
        scopes.push(current.clone());
    }
    if let Some(s) = args.session {
        current = current.session(s);
        scopes.push(current.clone());
    }
    if let Some(a) = args.agent {
        current = current.agent(a);
        scopes.push(current.clone());
    }
    let checkpoint_scope = current.session.as_ref().map(|_| current.clone());
    let access = if args.operator {
        Access::operator(scopes)?
    } else if args.read_only {
        Access::readonly(scopes)?
    } else {
        Access::agent(scopes)?
    };
    let mut store = Store::open(&args.db)?;
    let snapshot = match &args.workspace_root {
        Some(root) => workspace::snapshot(root, store.dependency_paths(&access)?)?,
        None => Snapshot::default(),
    };
    match args.command {
        Command::Status => print(&store.status(&access)?),
        Command::Lens {
            trace,
            after,
            limit,
        } => print(&store.lens_snapshot(
            &access,
            trace.as_deref(),
            after.as_deref(),
            limit,
            &snapshot,
        )?),
        Command::Events {
            after,
            limit,
            jsonl,
        } => {
            let page = store.event_page(&access, after, limit)?;
            if jsonl {
                use std::io::Write;
                let mut out = io::stdout().lock();
                for event in &page.events {
                    serde_json::to_writer(&mut out, event)?;
                    out.write_all(b"\n")?;
                }
                Ok(())
            } else {
                print(&page)
            }
        }
        Command::Preferences {
            id,
            revision,
            pinned,
            suppressed,
        } => print(&store.set_preferences(&access, &id, revision, pinned, suppressed)?),
        Command::History {
            id,
            known_at,
            valid_at,
        } => print(&store.memory_as_of(&access, &id, known_at, valid_at)?),
        Command::List { after, limit } => print(&store.list(&access, after.as_deref(), limit)?),
        Command::Propose { request, file } => {
            print(&store.capture(&access, &request, read_json(&file)?)?)
        }
        Command::Search {
            query,
            limit,
            include_stale,
            embedding,
        } => {
            let embedding = embedding.as_deref().map(read_json).transpose()?;
            print(&store.recall(
                &access,
                &Recall {
                    query,
                    limit,
                    include_stale,
                    embedding,
                    snapshot,
                    ..Recall::default()
                },
            )?)
        }
        Command::Get { id } => {
            let memory = store.get(&access, &id)?;
            print(
                &serde_json::json!({"freshness":store.freshness(&access,&memory,&snapshot)?,"memory":memory}),
            )
        }
        Command::Approve {
            id,
            revision,
            validation,
        } => {
            let receipt: Option<ValidationReceipt> =
                validation.as_deref().map(read_json).transpose()?;
            print(&store.approve(&access, &id, revision, receipt.as_ref(), &snapshot)?)
        }
        Command::Reject { id, revision } => print(&store.reject(&access, &id, revision)?),
        Command::Supersede {
            old,
            new,
            old_revision,
            new_revision,
            validation,
        } => {
            let receipt: Option<ValidationReceipt> =
                validation.as_deref().map(read_json).transpose()?;
            print(&store.supersede(
                &access,
                (&old, old_revision),
                (&new, new_revision),
                receipt.as_ref(),
                &snapshot,
            )?)
        }
        Command::Forget { id, revision } => print(&store.forget(&access, &id, revision)?),
        Command::Link { from, to, relation } => {
            store.link(&access, &from, &to, Relation::parse(&relation)?)?;
            print(&serde_json::json!({"linked":true}))
        }
        Command::Embed {
            id,
            content_hash,
            file,
        } => {
            store.set_embedding(&access, &id, &content_hash, &read_json(&file)?)?;
            print(&serde_json::json!({"stored":true}))
        }
        Command::Context { query, max_bytes } => {
            let report = store.recall(
                &access,
                &Recall {
                    query,
                    snapshot,
                    limit: 64,
                    ..Recall::default()
                },
            )?;
            print(&compile_context(
                &report.hits,
                &ByteCounter,
                &ContextBudget {
                    max_units: max_bytes,
                    max_bytes: max_bytes.min(64 * 1024),
                    max_entries: 32,
                },
            )?)
        }
        Command::CheckpointSave {
            file,
            expected_revision,
        } => print(&store.save_checkpoint(
            &access,
            read_json(&file)?,
            expected_revision,
            &snapshot,
        )?),
        Command::Resume { key } => {
            let scope =
                checkpoint_scope.ok_or_else(|| Error::Invalid("--session is required".into()))?;
            print(&store.resume(&access, &scope, &key, &snapshot)?)
        }
        Command::ImportMarkdown { file } => {
            let text = read_bounded(&file, 1024 * 1024)?;
            let uri = format!("file://{}", std::fs::canonicalize(&file)?.display());
            print(&import::markdown(
                &mut store, &access, &current, &uri, &text,
            )?)
        }
        Command::ImportJsonl { file } => {
            let text = read_bounded(&file, 16 * 1024 * 1024)?;
            print(&import::jsonl(&mut store, &access, &current, &text)?)
        }
        Command::Export => {
            store.export_jsonl(&access, io::stdout().lock())?;
            Ok(())
        }
        Command::Reindex => {
            store.reindex(&access)?;
            print(&serde_json::json!({"reindexed":true}))
        }
        Command::Expire => print(&serde_json::json!({"marked_stale":store.expire(&access)?})),
        Command::Serve => ToolServer::new(
            store,
            access,
            current,
            checkpoint_scope,
            args.workspace_root,
        )?
        .serve(io::stdin().lock(), io::stdout().lock()),
    }
}
fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("{}: {}", error.code(), error);
        std::process::exit(2);
    }
}
