extern crate notify;
extern crate notify_rust;
extern crate users;

use notify::{raw_watcher, Op, RawEvent, RecursiveMode, Watcher};
use notify_rust::{Notification, NotificationHandle};
use users::{get_user_by_uid, User};

use std::collections::HashMap;
use std::fs::metadata;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::channel;

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

struct PTY {
    id: u32,
    owner: User,
    notification: NotificationHandle,
}

struct App {
    pts: HashMap<u32, PTY>,
}

impl App {
    fn new() -> Self {
        App {
            pts: HashMap::default(),
        }
    }

    fn handle_event(&mut self, path: PathBuf, op: Op) -> AnyResult<()> {
        let path = Path::new(&path);
        match op {
            Op::CREATE => {
                let stat = metadata(path)?;
                if let (Some(name), Some(user)) = (path.file_name(), get_user_by_uid(stat.uid())) {
                    let id = u32::from_str_radix(name.to_str().unwrap(), 10)?;

                    let msg = format!("{} was opened for {}", id, user.name().to_str().unwrap(),);
                    let notification = Notification::new()
                        .appname("PTY Notify")
                        .summary("PTY Opened")
                        .body(&msg)
                        .show()?;
                    println!("{}", msg);

                    self.pts.insert(
                        id,
                        PTY {
                            id,
                            owner: user,
                            notification,
                        },
                    );
                }
            }
            Op::REMOVE => {
                let id = u32::from_str_radix(path.file_name().unwrap().to_str().unwrap(), 10)?;

                let msg = if let Some(mut pty) = self.pts.remove(&id) {
                    let msg = format!("{} was closed ({})", id, pty.owner.name().to_string_lossy());
                    pty.notification.summary("PTY Closed").body(&msg);
                    pty.notification.update();
                    msg
                } else {
                    format!("{} was closed", id)
                };
                println!("{}", msg);
            }
            _ => {}
        };
        Ok(())
    }
}

fn main() -> AnyResult<()> {
    let mut app = App::new();

    let (tx, rx) = channel();

    let mut watcher = raw_watcher(tx)?;

    watcher.watch("/dev/pts", RecursiveMode::NonRecursive)?;

    loop {
        match rx.recv() {
            Ok(RawEvent {
                path: Some(path),
                op: Ok(op),
                cookie: _,
            }) => app.handle_event(path, op)?,
            Err(e) => eprintln!("error: {:?}", e),
            _ => {}
        }
    }
}
