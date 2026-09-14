//! Live view transport replacing the app-local QuickJS Worker. Only immutable
//! projections cross back to Ratatui; network, raster and encoding stay here.
use super::{graphics, owner};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{self, Read},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone, Deserialize, Serialize)]
pub struct Style {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub alpha: f64,
    pub hollow: bool,
    pub channel: String,
    pub arch: String,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Pose {
    pub points: Vec<[f64; 2]>,
    pub style: Style,
    pub state: Value,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Activity {
    pub label: String,
    pub tool: Option<String>,
    pub observed: bool,
    pub parallel: usize,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub version: u8,
    pub identity: String,
    pub epoch: String,
    pub tick: u64,
    pub cursor: u64,
    pub source: String,
    pub source_revision: u64,
    pub time_ms: f64,
    pub digest: String,
    pub behaviour: String,
    pub producer_connected: bool,
    pub storage_available: bool,
    pub audio_owner: Option<String>,
    pub audio_unavailable: bool,
    pub points: Vec<[f64; 2]>,
    pub style: Style,
    pub state: Value,
    pub still: Pose,
    #[serde(default)]
    pub appearance: super::appearance::Appearance,
    #[serde(default)]
    pub activity: Option<Activity>,
}
impl Scene {
    fn valid(&self) -> bool {
        self.version == 1
            && self.time_ms.is_finite()
            && self.points.len() == 980
            && self.still.points.len() == 980
            && self
                .points
                .iter()
                .chain(&self.still.points)
                .flatten()
                .all(|p| p.is_finite() && p.abs() <= 8.0)
    }
}
#[derive(Clone)]
pub struct Presentation {
    pub client: String,
    pub scene: Scene,
    pub cells: Vec<u8>,
    pub image: Option<Vec<u8>>,
    pub width: u16,
    pub height: u16,
    pub created: Instant,
    pub frame_changed: Instant,
    pub render_ms: f64,
    pub bytes: usize,
}
#[derive(Clone, Default)]
pub struct View {
    pub width: u16,
    pub height: u16,
    pub cell_width: f64,
    pub cell_height: f64,
    pub motion: bool,
    pub pixels: bool,
    pub visible: bool,
    pub waiting: bool,
    pub sound: bool,
}
#[derive(Clone)]
pub enum Command {
    Observe(String),
    Select,
    Browser,
    Window,
    Export,
}
pub enum Notice {
    Exported(PathBuf),
    Message(String),
}
pub struct Worker {
    pub tx: mpsc::SyncSender<Command>,
    pub latest: Arc<Mutex<Option<Presentation>>>,
    pub view: Arc<Mutex<View>>,
    pub notices: mpsc::Receiver<Notice>,
}

pub struct Client {
    pub descriptor: owner::Descriptor,
    http: reqwest::blocking::Client,
}
impl Client {
    pub fn connect() -> io::Result<Self> {
        let root = owner::directory()?;
        let http = reqwest::blocking::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_millis(500))
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(io::Error::other)?;
        let attempt = || -> io::Result<Self> {
            Ok(Self {
                descriptor: owner::descriptor(&root)?,
                http: http.clone(),
            })
        };
        if let Ok(client) = attempt()
            && client.get("/v1/frame").is_ok()
        {
            return Ok(client);
        }
        #[cfg(not(test))]
        {
            use std::process::{Command as Process, Stdio};
            let mut process = Process::new(std::env::current_exe()?);
            process
                .args(["pet", "serve"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                unsafe {
                    process.pre_exec(|| {
                        if libc::setsid() < 0 {
                            return Err(io::Error::last_os_error());
                        }
                        Ok(())
                    });
                }
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                process.creation_flags(0x08000000 | 0x00000008);
            }
            let mut child = process.spawn()?;
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        let began = Instant::now();
        while began.elapsed() < Duration::from_secs(5) {
            if let Ok(client) = attempt()
                && client.get("/v1/frame").is_ok()
            {
                return Ok(client);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(io::Error::other(
            "Shared pet unavailable. Run codewhale pet serve; existing recordings are preserved.",
        ))
    }
    pub fn get(&self, path: &str) -> io::Result<Value> {
        self.request(path, None)
    }
    pub fn post(&self, path: &str, body: &Value) -> io::Result<Value> {
        self.request(path, Some(body))
    }
    fn request(&self, path: &str, body: Option<&Value>) -> io::Result<Value> {
        let url = format!("http://127.0.0.1:{}{path}", self.descriptor.port);
        let request = if let Some(body) = body {
            self.http.post(url).json(body)
        } else {
            self.http.get(url)
        };
        let response = request
            .bearer_auth(&self.descriptor.token)
            .send()
            .map_err(io::Error::other)?;
        let status = response.status();
        let success = status.is_success();
        let bound = if path == "/v1/export" {
            super::persistence::MAX_EXPORT_BYTES
        } else {
            8 * 1024 * 1024
        };
        let mut bytes = Vec::new();
        response.take(bound as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > bound {
            return Err(io::Error::other("Pet response exceeds its bound"));
        }
        let value: Value = serde_json::from_slice(&bytes)?;
        if !success {
            let message = value["error"]
                .as_str()
                .unwrap_or("Shared pet connection failed");
            return Err(io::Error::new(
                if status == reqwest::StatusCode::CONFLICT && !message.contains("storage") {
                    io::ErrorKind::InvalidInput
                } else {
                    io::ErrorKind::Other
                },
                message,
            ));
        }
        Ok(value)
    }
    pub fn open_browser(&self) -> io::Result<()> {
        let url = format!(
            "http://127.0.0.1:{}/#{}",
            self.descriptor.port, self.descriptor.token
        );
        webbrowser::open(&url).map_err(io::Error::other)
    }
    pub fn open_window(&self) -> io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            use std::process::{Command as Process, Stdio};
            let mut process = Process::new("open");
            if let Some(path) = std::env::var_os("CODEWHALE_PET_APP") {
                process.arg(path);
            } else {
                process.args(["-a", "Codewhale Pet"]);
            }
            process
                .args(["--args", "--companion"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if process.status()?.success() {
                return Ok(());
            }
            Err(io::Error::other(
                "Build or install the Codewhale Pet app to open its companion window",
            ))
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(io::Error::other(
                "The native companion window is currently available on macOS",
            ))
        }
    }
}
impl Worker {
    pub fn start(session: Option<String>) -> io::Result<Self> {
        let (tx, rx) = mpsc::sync_channel(128);
        let (latest, (notices_tx, notices)) = (Arc::new(Mutex::new(None)), mpsc::sync_channel(16));
        let output = latest.clone();
        let view = Arc::new(Mutex::new(View {
            width: 40,
            height: 8,
            ..View::default()
        }));
        let settings = view.clone();
        std::thread::Builder::new()
            .name("pet-view".into())
            .spawn(move || {
                if let Err(e) = run(rx, &output, &notices_tx, &settings, session) {
                    let _ = notices_tx.try_send(Notice::Message(e.to_string()));
                }
            })?;
        Ok(Self {
            tx,
            latest,
            notices,
            view,
        })
    }
}
fn run(
    rx: mpsc::Receiver<Command>,
    output: &Mutex<Option<Presentation>>,
    notices: &mpsc::SyncSender<Notice>,
    settings: &Mutex<View>,
    session: Option<String>,
) -> io::Result<()> {
    use sha2::{Digest, Sha256};
    let source = session.as_ref().map(|s| {
        format!(
            "session:{}",
            Sha256::digest(s.as_bytes())
                .iter()
                .take(10)
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        )
    });
    let mut renderer = graphics::Renderer::default();
    let id = uuid::Uuid::new_v4().to_string();
    let mut client = Client::connect()?;

    let mut scene: Option<Scene> = None;
    let mut previous = None;
    let mut changed = Instant::now();
    let mut fetched = Instant::now() - Duration::from_secs(1);
    let mut encoded = Instant::now() - Duration::from_secs(1);
    let mut events = Vec::new();
    let mut producer_seq = None;
    let mut sequence = 0u64;
    let mut action: Option<Value> = None;
    let mut last_action = Instant::now() - Duration::from_secs(1);
    let mut last_produce = Instant::now();
    let mut last_audio = Instant::now();
    let mut last_failure = Instant::now() - Duration::from_secs(10);
    loop {
        let view = settings
            .lock()
            .map_err(|_| io::Error::other("Pet view lock failed"))?
            .clone();
        match rx.recv_timeout(Duration::from_millis(2)) {
            Ok(command) => match command {
                Command::Observe(text) => {
                    if events.len() < 64 {
                        events.push(serde_json::from_str::<Value>(&text)?)
                    } else {
                        events.clear();
                        producer_seq = None;
                    }
                }
                Command::Select => {
                    if let (Some(s), Some(source)) = (&scene, &source)
                        && action.is_none()
                    {
                        action = Some(
                            json!({"identity":s.identity,"client":id,"seq":sequence+1,"source_revision":s.source_revision,"action":{"kind":"select","source":source}}),
                        );
                    } else {
                        let _ = notices.try_send(Notice::Message(
                            "Save this session and wait for the pet connection before selecting its source.".into(),
                        ));
                    }
                }
                Command::Browser => {
                    if let Err(e) = client.open_browser() {
                        let _ = notices.try_send(Notice::Message(e.to_string()));
                    }
                }
                Command::Window => {
                    if let Err(e) = client.open_window() {
                        let _ = notices.try_send(Notice::Message(e.to_string()));
                    }
                }
                Command::Export => {
                    let result = client.get("/v1/export").and_then(|r| {
                        super::persistence::export(
                            session.as_deref().ok_or_else(|| {
                                io::Error::other("Save the terminal session before exporting")
                            })?,
                            &serde_json::to_vec(&r)?,
                        )
                    });
                    let _ = notices.try_send(match result {
                        Ok(path) => Notice::Exported(path),
                        Err(e) => Notice::Message(e.to_string()),
                    });
                }
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = client.post("/v1/audio", &json!({"client":id,"enabled":false}));
                return Ok(());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if fetched.elapsed() >= Duration::from_millis(if view.visible { 30 } else { 400 }) {
            match client
                .get("/v1/frame")
                .and_then(|value| serde_json::from_value::<Scene>(value).map_err(io::Error::other))
            {
                Ok(next) if next.valid() => {
                    if scene
                        .as_ref()
                        .is_none_or(|s| s.epoch != next.epoch || s.identity != next.identity)
                    {
                        previous = None;
                        changed = Instant::now();
                        producer_seq = None;
                        events.clear();
                    } else if scene.as_ref().is_some_and(|s| s.tick != next.tick) {
                        previous = scene.clone();
                        changed = Instant::now();
                    }
                    if next.source == "unattached"
                        && let Some(source) = &source
                        && action.is_none()
                    {
                        action = Some(
                            json!({"identity":next.identity,"client":id,"seq":sequence+1,"source_revision":next.source_revision,"action":{"kind":"select","source":source}}),
                        );
                    }
                    scene = Some(next);
                    fetched = Instant::now();
                }
                _ => {
                    fetched = Instant::now();
                    events.clear();
                    producer_seq = None;
                    if last_failure.elapsed() > Duration::from_secs(3) {
                        last_failure = Instant::now();
                        let _ = notices.try_send(Notice::Message(
                            "Shared pet reconnecting · unobserved".into(),
                        ));
                        if let Ok(next) = Client::connect() {
                            client = next;
                        }
                    }
                    continue;
                }
            }
        }
        let Some(s) = &scene else { continue };
        if last_action.elapsed() >= Duration::from_millis(250)
            && let Some(pending) = &action
        {
            last_action = Instant::now();
            match client.post("/v1/action", pending) {
                Ok(_) => {
                    let selected = pending["action"]["kind"] == "select";
                    sequence += 1;
                    action = None;
                    if selected {
                        producer_seq = None;
                        events.clear();
                    }
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::InvalidInput {
                        action = None;
                    }
                    if last_failure.elapsed() > Duration::from_secs(3) {
                        last_failure = Instant::now();
                        let _ = notices.try_send(Notice::Message(e.to_string()));
                    }
                }
            }
        }
        if source.as_deref() == Some(s.source.as_str())
            && last_produce.elapsed() >= Duration::from_millis(100)
        {
            let seq = producer_seq.map_or(0, |seq| seq + 1);
            if seq == 0 {
                events.clear();
            }
            let body = json!({"identity":s.identity,"epoch":s.epoch,"client":id,"source":s.source,"source_revision":s.source_revision,"seq":seq,"waiting":view.waiting,"events":events});
            producer_seq = client.post("/v1/producer", &body).ok().map(|_| seq);
            events.clear();
            last_produce = Instant::now();
        } else if source.as_deref() != Some(s.source.as_str()) {
            events.clear();
            producer_seq = None;
        }
        if last_audio.elapsed() >= Duration::from_millis(500) {
            let _=client.post("/v1/audio",&json!({"client":id,"enabled":view.sound&&view.visible&&changed.elapsed()<Duration::from_millis(500)}));
            last_audio = Instant::now();
        }
        if view.visible
            && encoded.elapsed()
                >= Duration::from_millis(if view.motion && view.pixels {
                    16
                } else if view.motion {
                    33
                } else {
                    200
                })
        {
            let began = Instant::now();
            let mut pose = if view.motion {
                Pose {
                    points: s.points.clone(),
                    style: s.style.clone(),
                    state: s.state.clone(),
                }
            } else {
                s.still.clone()
            };
            if !s.producer_connected {
                pose.style.hollow = true;
            }
            let fraction = if view.motion {
                (changed.elapsed().as_secs_f64() * 30.0).clamp(0.0, 1.0)
            } else {
                1.0
            };
            renderer.set_appearance(&s.appearance);
            let (cells, image) = renderer.render(
                &pose,
                previous.as_ref().filter(|_| view.motion),
                fraction,
                &view,
                if view.motion { s.time_ms / 1000.0 } else { 0.0 },
            )?;
            let bytes = image.as_ref().map_or(0, Vec::len);
            if let Ok(mut slot) = output.lock() {
                *slot = Some(Presentation {
                    client: id.clone(),
                    scene: s.clone(),
                    cells,
                    image,
                    width: view.width,
                    height: view.height,
                    created: Instant::now(),
                    frame_changed: changed,
                    render_ms: began.elapsed().as_secs_f64() * 1000.0,
                    bytes,
                });
            }
            encoded = began;
        }
    }
}
