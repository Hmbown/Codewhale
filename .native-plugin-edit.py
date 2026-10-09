from pathlib import Path
import json
r=Path('.')
def e(path, old, new):
 p=r/path;s=p.read_text();assert old in s,(path,old[:80]);p.write_text(s.replace(old,new,1))
def b(path, anchor, text): e(path,anchor,text+anchor)
b('crates/tui/src/plugins/install/mod.rs','        if let Some(path) = trimmed.strip_prefix("path:") {','''        if let Some(spec) = trimmed.strip_prefix("git:") {
            let spec = spec.strip_prefix("https://").unwrap_or(spec);
            let spec = spec.strip_prefix("github.com/")
                .context("git plugin sources must use git:github.com/owner/repo[@ref]")?;
            let (repo, revision) = spec.rsplit_once('@')
                .map_or((spec, None), |(repo, revision)| (repo, Some(revision)));
            let repo = repo.strip_suffix(".git").unwrap_or(repo);
            let valid = |part: &str| !part.is_empty() && part != "." && part != ".."
                && part.bytes().all(|ch| ch.is_ascii_alphanumeric() || b"-_.".contains(&ch));
            let parts: Vec<_> = repo.split('/').collect();
            if parts.len() != 2 || !parts.iter().all(|part| valid(part)) {
                bail!("git plugin source must name one GitHub owner/repository");
            }
            return match revision {
                None => Ok(Self::Remote(InstallSource::GitHubRepo(repo.to_owned()))),
                Some(revision) if valid(revision) => Ok(Self::Remote(InstallSource::DirectUrl(
                    format!("https://github.com/{repo}/archive/{revision}.tar.gz")
                ))),
                _ => bail!("git plugin ref must be a single safe tag, branch name or commit"),
            };
        }
        if let Some(spec) = trimmed.strip_prefix("npm:") {
            let (package, version) = spec.rsplit_once('@')
                .context("npm plugin sources require an exact version: npm:package@1.2.3")?;
            let valid = |part: &str| !part.is_empty() && part != "." && part != ".."
                && part.bytes().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || b"-_.".contains(&ch));
            let name = if let Some(scoped) = package.strip_prefix('@') {
                let (scope, name) = scoped.split_once('/').context("invalid npm scope/name")?;
                if !valid(scope) || !valid(name) { bail!("invalid npm scope/name"); }
                name
            } else {
                if !valid(package) { bail!("invalid npm package name"); }
                package
            };
            let parsed = semver::Version::parse(version).context("npm plugin version must be exact")?;
            if parsed.to_string() != version { bail!("npm plugin version must be canonical"); }
            return Ok(Self::Remote(InstallSource::DirectUrl(
                format!("https://registry.npmjs.org/{package}/-/{name}-{version}.tgz")
            )));
        }
''')
b('crates/tui/src/plugins/mutation.rs','fn user_plugins_dir(registry: &PluginRegistry) -> Result<PathBuf> {','''pub async fn install_from_cli(
    source: String,
    config: crate::config::Config,
    mut registry: PluginRegistry,
) -> Result<()> {
    let runtime = tokio::runtime::Handle::current();
    let policy = crate::plugins::activation::extension_host_policy_enabled();
    let receipt = tokio::task::spawn_blocking(move || {
        let _scope = crate::plugins::activation::PolicyScope::propagate(policy);
        let network = config.network.map(|network| network.into_runtime()).unwrap_or_default();
        let source = PluginInstallSource::parse(&source)?;
        runtime.block_on(execute(
            PluginMutationRequest::Install { source },
            &PluginMutationContext { network: &network, max_size: install::DEFAULT_MAX_SIZE_BYTES },
            &mut registry,
        ))
    }).await??;
    match receipt.outcome {
        PluginMutationOutcome::Installed => {
            println!("Installed {} (disabled, untrusted).", receipt.name);
            println!("Open Codewhale, review `/plugin trust {}`, then enable it with `/plugin enable {}`. Start a new session after enabling provider declarations.", receipt.name, receipt.name);
            Ok(())
        }
        PluginMutationOutcome::NeedsApproval(host) => bail!("Network approval required for {host}; use /network allow {host}, then retry"),
        PluginMutationOutcome::NetworkDenied(host) => bail!("Network access denied for {host}"),
        _ => bail!("Unexpected plugin install outcome"),
    }
}

''')
b('crates/cli/src/lib.rs','    Setup(TuiPassthroughArgs),','''    #[command(about = "Install a plugin bundle without trusting or enabling it")]
    Install(TuiPassthroughArgs),
''')
b('crates/cli/src/lib.rs','        Some(Commands::Setup(args)) => {','''        Some(Commands::Install(args)) => {
            let resolved_runtime = resolve_runtime_for_diagnostic_dispatch(&store, &runtime_overrides);
            run_tui_in_process(&cli, &resolved_runtime, tui_args("install", args))
        }
''')
b('crates/tui/src/lib.rs','    Setup(SetupArgs),','''    #[command(about = "Install a local, GitHub or version-pinned npm plugin bundle")]
    Install { source: String },
''')
b('crates/tui/src/lib.rs','            Commands::Setup(args) => {','''            Commands::Install { source } => {
                let config = load_config_from_cli(&cli)?;
                crate::plugins::mutation::install_from_cli(source, config, (*plugin_registry).clone()).await
            }
''')
b('crates/tui/src/tui/app/types.rs','    StartOrcarouterPkceLogin,','''    StartPluginLogin { provider: String },
    StartPluginLogout { provider: String },
''')
b('crates/tui/src/tui/views/mod.rs','    ProviderPickerOrcarouterOAuthRequested,','''    ProviderPickerPluginOAuthRequested { provider: String },
''')
b('crates/tui/src/tui/ui/apply.rs','        AppAction::StartOrcarouterPkceLogin => {','''        AppAction::StartPluginLogin { provider } => {
            run_plugin_oauth_from_tui(terminal, app, config, provider, false).await?;
        }
        AppAction::StartPluginLogout { provider } => {
            run_plugin_oauth_from_tui(terminal, app, config, provider, true).await?;
        }
''')
b('crates/tui/src/tui/ui/handlers.rs','            ViewEvent::ProviderPickerOrcarouterOAuthRequested => {','''            ViewEvent::ProviderPickerPluginOAuthRequested { provider } => {
                run_plugin_oauth_from_tui(terminal, app, config, provider, false).await?;
            }
''')
e('crates/tui/src/commands/groups/config/config.rs','''    let token = raw.split_whitespace().next().unwrap_or("");
    match token {
        "" | "status" => CommandResult::message(login_status_text(app)),''','''    if raw.split_whitespace().count() > 1 {
        return CommandResult::error("Usage: /login [status|account|key|<provider>]");
    }
    match raw {
        "" => CommandResult::action(AppAction::OpenProviderPicker),
        "status" => CommandResult::message(login_status_text(app)),''')
e('crates/tui/src/commands/groups/config/config.rs','''        other => CommandResult::error(format!(
            "Usage: /login [status|account|key]\\nUnknown argument: {other}"
        )),''','''        provider if app.plugin_registry.active_plugins().any(|plugin| plugin.manifest.providers.contains_key(provider)) => {
            CommandResult::action(AppAction::StartPluginLogin { provider: provider.to_owned() })
        }
        other => CommandResult::error(format!(
            "Usage: /login [status|account|key|<provider>]\\nUnknown or inactive provider: {other}"
        )),''')
e('crates/tui/src/commands/groups/config/config.rs','Then `/login` to confirm the session landed.','Then `/login status` to confirm the session landed.')
e('crates/tui/src/commands/groups/config/config.rs','''/// Unified login status. Account device flow stays on the CLI so this
/// command never freezes the TUI and never invents a second OAuth broker.
/// The internal cloud-agent credential is not user surface: membership
/// (`codewhale login`) is the only door, never a provider key.
''','')
e('crates/tui/src/commands/groups/config/mod.rs','usage: "/login [status|account|key]",','usage: "/login [status|account|key|<provider>]",')
e('crates/tui/src/commands/groups/config/mod.rs','usage: "/logout",','usage: "/logout [<provider>]",')
e('crates/tui/src/commands/groups/config/mod.rs','''        "logout" => config::logout(app),''','''        "logout" => match arg.map(str::trim).filter(|arg| !arg.is_empty()) {
            None => config::logout(app),
            Some(provider) if app.plugin_registry.active_plugins().any(|plugin| plugin.manifest.providers.contains_key(provider)) =>
                CommandResult::action(crate::tui::app::AppAction::StartPluginLogout { provider: provider.to_owned() }),
            Some(_) => CommandResult::error("Usage: /logout [<enabled-plugin-provider>]"),
        },''')
b('crates/tui/src/tui/provider_picker.rs','    fn key_entry_is_oauth_locked(&self) -> bool {','''    fn selected_plugin_provider(&self) -> Option<String> {
        let identity = self.selected_identity()?;
        crate::plugins::providers::plugin_auth_entry(&self.route_config, identity.key.as_str())
            .ok().map(|_| identity.key.as_str().to_owned())
    }

''')
e('crates/tui/src/tui/provider_picker.rs','''    fn key_entry_is_oauth_locked(&self) -> bool {
        self.selected_provider()''','''    fn key_entry_is_oauth_locked(&self) -> bool {
        if self.selected_plugin_provider().is_some() { return true; }
        self.selected_provider()''')
b('crates/tui/src/tui/provider_picker.rs','''        let provider = self.selected_provider();
        if provider == ProviderKind::Custom && !self.rows[self.selected_idx].is_configured {''','''        if let Some(provider) = self.selected_plugin_provider() {
            return if self.selected_has_key() && !self.selected_credential_rejected() {
                ViewAction::EmitAndClose(ViewEvent::ProviderPickerOpenModels {
                    identity: self.selected_identity().expect("admitted plugin provider"),
                })
            } else {
                ViewAction::EmitAndClose(ViewEvent::ProviderPickerPluginOAuthRequested { provider })
            };
        }
''')
e('crates/tui/src/tui/provider_picker.rs','''    fn render_key_entry(&self, area: Rect, buf: &mut Buffer) {
        let row''','''    fn render_key_entry(&self, area: Rect, buf: &mut Buffer) {
        if let Some(provider) = self.selected_plugin_provider() {
            let block = Block::default().title(provider).borders(Borders::ALL);
            let inner = block.inner(area);
            block.render(area, buf);
            Paragraph::new(self.tr(MessageId::PluginOAuthBrowser).into_owned())
                .wrap(Wrap { trim: true }).render(inner, buf);
            return;
        }
        let row''')
e('crates/tui/src/tui/provider_picker.rs','''                KeyCode::Enter => {
                    if self.selected_provider() == ProviderKind::OpenaiCodex {''','''                KeyCode::Enter => {
                    if let Some(provider) = self.selected_plugin_provider() {
                        return ViewAction::EmitAndClose(ViewEvent::ProviderPickerPluginOAuthRequested { provider });
                    }
                    if self.selected_provider() == ProviderKind::OpenaiCodex {''')
b('crates/tui/src/plugins/providers.rs','pub fn apply_providers(config: &mut Config, registry: &PluginRegistry) -> anyhow::Result<()> {','''pub(crate) fn is_account_catalog_scope(provider: &str, fingerprint: &str) -> bool {
    STARTUP_REGISTRY.get().is_some_and(|registry| registry.active_plugins().any(|plugin| {
        plugin.manifest.providers.iter().any(|(name, declaration)| {
            provider == format!("custom:{name}")
                && fingerprint == codewhale_config::catalog::base_url_fingerprint(&declaration.base_url)
        })
    }))
}

''')
e('crates/tui/src/provider_catalog_live.rs','''fn is_account_scoped_scope(provider: &str, fingerprint: &str) -> bool {
    provider.starts_with("codewhale:")''','''fn is_account_scoped_scope(provider: &str, fingerprint: &str) -> bool {
    crate::plugins::providers::is_account_catalog_scope(provider, fingerprint)
        || provider.starts_with("codewhale:")''')
b('crates/tui/src/tui/ui/event_loop.rs','pub(crate) async fn run_chatgpt_pkce_login_from_tui(','''pub(crate) async fn run_plugin_oauth_from_tui(
    terminal: &mut AppTerminal,
    app: &mut App,
    config: &mut Config,
    provider: String,
    logout: bool,
) -> Result<()> {
    let entry = match crate::plugins::providers::plugin_auth_entry(config, &provider) {
        Ok(entry) => entry,
        Err(error) => {
            app.push_status_toast(error.to_string(), StatusToastLevel::Error, Some(App::STICKY_ERROR_TTL_MS));
            return Ok(());
        }
    };
    let identity = config.resolve_provider_pin_identity(&provider).map_err(anyhow::Error::msg)?;
    pause_terminal(terminal, app.use_alt_screen(), app.use_mouse_capture, app.use_bracketed_paste)?;
    let result: Result<()> = async {
        let base_url = entry.base_url.clone().context("Missing plugin endpoint")?;
        let oauth = entry.oauth.clone().context("Missing plugin OAuth declaration")?;
        let authority = entry.plugin_authority.clone().context("Missing plugin review")?;
        if logout {
            let provider = provider.clone();
            let policy = crate::plugins::activation::extension_host_policy_enabled();
            tokio::task::spawn_blocking(move || {
                let _scope = crate::plugins::activation::PolicyScope::propagate(policy);
                crate::plugins::providers::verify_provider_binding(&authority, &provider, &base_url, &oauth, None)
                    .map_err(anyhow::Error::msg)?;
                crate::oauth::plugin_oauth_logout(&provider, &base_url, &oauth)
            }).await??;
        } else {
            crate::oauth::plugin_oauth_login(provider.clone(), base_url, oauth, authority).await?;
        }
        let ticket = crate::provider_catalog_live::begin_refresh_for_identity(
            identity.provider, &provider, &config.base_url_for_route(&identity),
        );
        if !logout {
            let mut scoped = config.clone();
            scoped.scope_to_provider_identity(&identity).map_err(anyhow::Error::msg)?;
            let client = crate::client::CodewhaleClient::for_catalog_refresh(&scoped)?;
            let delta = client.fetch_catalog_delta().await.map_err(|error| anyhow::anyhow!("{error:?}"))?;
            crate::provider_catalog_live::record_success_if_current(&ticket, delta)
                .context("Catalog refresh superseded")?;
        }
        Ok(())
    }.await;
    resume_terminal(terminal, app.use_alt_screen(), app.use_mouse_capture, app.use_bracketed_paste, app.synchronized_output_enabled)?;
    match result {
        Ok(()) => {
            let message = if logout { MessageId::PluginOAuthLocalLogout } else { MessageId::PluginOAuthReady };
            app.push_status_toast(tr(app.ui_locale, message).replace("{provider}", &provider), StatusToastLevel::Success, Some(8_000));
            if !logout { open_model_picker_for_provider(app, config, &identity); }
        }
        Err(error) => app.push_status_toast(format!("{provider}: {error}"), StatusToastLevel::Error, Some(App::STICKY_ERROR_TTL_MS)),
    }
    app.needs_redraw = true;
    Ok(())
}

''')
t={
'en':['Press Enter to sign in with your browser. Esc returns. No API key is needed.','{provider}: signed in. Choose a model.','{provider}: local sign-in removed. Remote access is unchanged.'],
'zh-Hans':['按 Enter 打开浏览器登录，按 Esc 返回。不需要 API Key。','{provider}：已登录，请选择模型。','{provider}：已删除本机登录凭据，未撤销服务端授权。'],
'zh-Hant':['按 Enter 開啟瀏覽器登入，按 Esc 返回。不需要 API Key。','{provider}：已登入，請選擇模型。','{provider}：已刪除本機登入憑證，未撤銷伺服器授權。'],
'ja':['Enter でブラウザーからログインします。Esc で戻ります。API キーは不要です。','{provider}：ログインしました。モデルを選択してください。','{provider}：この端末の認証情報を削除しました。サーバーの認可は解除されていません。'],
'ko':['Enter를 눌러 브라우저에서 로그인하세요. Esc를 누르면 돌아갑니다. API 키는 필요하지 않습니다.','{provider}: 로그인했습니다. 모델을 선택하세요.','{provider}: 이 기기의 로그인 정보를 삭제했습니다. 서버 권한은 유지됩니다.'],
'fr':['Appuyez sur Entrée pour vous connecter dans le navigateur, ou Échap pour revenir. Aucune clé API nécessaire.','{provider} : connexion établie. Choisissez un modèle.','{provider} : connexion locale supprimée. L’accès distant reste inchangé.'],
'de':['Mit Eingabe im Browser anmelden, mit Esc zurück. Kein API-Schlüssel erforderlich.','{provider}: angemeldet. Wählen Sie ein Modell.','{provider}: lokale Anmeldung entfernt. Der Fernzugriff bleibt unverändert.'],
'es-419':['Presiona Intro para iniciar sesión en el navegador o Esc para volver. No se necesita una clave API.','{provider}: sesión iniciada. Elige un modelo.','{provider}: se eliminó la sesión local. El acceso remoto no cambió.'],
'pt-BR':['Pressione Enter para entrar pelo navegador ou Esc para voltar. Não é necessária uma chave de API.','{provider}: sessão iniciada. Escolha um modelo.','{provider}: sessão local removida. O acesso remoto não foi alterado.'],
'ca':['Prem Retorn per iniciar sessió al navegador o Esc per tornar. No cal cap clau API.','{provider}: sessió iniciada. Tria un model.','{provider}: sessió local eliminada. L’accés remot no ha canviat.'],
'ru':['Нажмите Enter для входа через браузер или Esc для возврата. Ключ API не нужен.','{provider}: вход выполнен. Выберите модель.','{provider}: локальные данные входа удалены. Удалённый доступ не изменён.'],
'uk':['Натисніть Enter для входу через браузер або Esc для повернення. Ключ API не потрібен.','{provider}: вхід виконано. Виберіть модель.','{provider}: локальні дані входу видалено. Віддалений доступ не змінено.'],
'vi':['Nhấn Enter để đăng nhập bằng trình duyệt hoặc Esc để quay lại. Không cần khóa API.','{provider}: đã đăng nhập. Hãy chọn mô hình.','{provider}: đã xóa thông tin đăng nhập cục bộ. Quyền truy cập từ xa không đổi.'],
'id':['Tekan Enter untuk masuk melalui peramban atau Esc untuk kembali. Kunci API tidak diperlukan.','{provider}: berhasil masuk. Pilih model.','{provider}: data masuk lokal dihapus. Akses jarak jauh tidak berubah.'],
'hi':['ब्राउज़र से साइन इन करने के लिए Enter दबाएँ। वापस जाने के लिए Esc दबाएँ। API कुंजी की ज़रूरत नहीं है।','{provider}: साइन इन हो गया। मॉडल चुनें।','{provider}: स्थानीय साइन इन हटाया गया। दूरस्थ पहुँच नहीं बदली है।']}
keys=['PluginOAuthBrowser','PluginOAuthReady','PluginOAuthLocalLogout']
for locale,values in t.items():
 p=r/f'crates/localization/locales/{locale}.json';s=p.read_text();data=json.loads(s);assert not any(k in data for k in keys)
 p.write_text(s.rstrip()[:-1].rstrip()+',\n'+',\n'.join('  '+json.dumps(k)+': '+json.dumps(v,ensure_ascii=False) for k,v in zip(keys,values))+'\n}\n')
for k in keys:
 b('crates/localization/src/lib.rs','    PlanHandoffProceed,',f'    {k},\n')
 b('crates/localization/src/lib.rs','    MessageId::PlanHandoffProceed,',f'    MessageId::{k},\n')
e('crates/tui/src/commands/mod.rs','let status = execute("/login", &mut app);','let status = execute("/login status", &mut app);')
e('crates/tui/src/commands/mod.rs','err.contains("Usage: /login [status|account|key]")','err.contains("Usage: /login [status|account|key|<provider>]")')
e('crates/tui/src/commands/mod.rs','        let key = execute("/login key", &mut app);','        assert_eq!(execute("/login", &mut app).action, Some(AppAction::OpenProviderPicker));\n        assert!(execute("/login status extra", &mut app).is_error);\n        let key = execute("/login key", &mut app);')
e('crates/tui/src/plugins/install/tests.rs','    assert!(PluginInstallSource::parse("path:").is_err());','''    assert!(PluginInstallSource::parse("path:").is_err());
    assert_eq!(PluginInstallSource::parse("git:github.com/owner/repo").unwrap(),
        PluginInstallSource::Remote(InstallSource::GitHubRepo("owner/repo".into())));
    for (spec, url) in [
        ("git:github.com/owner/repo@v1.2.3", "https://github.com/owner/repo/archive/v1.2.3.tar.gz"),
        ("npm:@owner/plugin@1.2.3", "https://registry.npmjs.org/@owner/plugin/-/plugin-1.2.3.tgz"),
        ("npm:plugin@1.2.3-alpha.1", "https://registry.npmjs.org/plugin/-/plugin-1.2.3-alpha.1.tgz"),
    ] {
        let source = PluginInstallSource::parse(spec).unwrap();
        assert_eq!(source, PluginInstallSource::Remote(InstallSource::DirectUrl(url.into())));
        assert_eq!(PluginInstallSource::parse(&plugin_spec_string(&source, None)).unwrap(), source);
    }
    for spec in ["git:gitlab.com/owner/repo", "git:github.com/owner/repo@../x",
        "git:github.com/owner/repo@", "npm:plugin", "npm:plugin@latest",
        "npm:plugin@^1.2.3", "npm:@owner/../plugin@1.2.3", "npm:../plugin@1.2.3"] {
        assert!(PluginInstallSource::parse(spec).is_err(), "{spec}");
    }''')
e('docs/PLUGINS.md','`/plugin install <spec>` accepts three source kinds:','''`codewhale install <spec>` and `/plugin install <spec>` use the same installer.
The shell entry does not start a model or require model credentials. Both keep
bundles disabled and untrusted until you explicitly review and enable them.

```sh
codewhale install git:github.com/example/plugin@v1.0.0
codewhale install npm:@example/plugin@1.0.0
codewhale install ./local-plugin
```

Git shorthand supports GitHub archives only; refs must be a single safe segment.
Use the existing HTTPS archive URL plus `#path=...` for a nested bundle.
Npm requires an exact version and a native plugin manifest in the tarball.
No git hooks, npm lifecycle scripts, dependency installation or Pi executable
extensions run. A ref names the requested revision, not an integrity signature;
review still binds the exact downloaded bytes. Tags can move: pin a commit for
immutable GitHub selection. Updates of version-pinned sources retain that version.
Network approval and archive safety checks are unchanged.

The original source forms remain available:''')
e('docs/PLUGIN_PROVIDERS.md','## Sign in and use the route','''## Sign in inside Codewhale

After installation, review and enable the bundle, then start a new session.
Run `/login` to open the provider picker, or `/login <provider-id>` to authorize
an enabled plugin directly. The host runs its existing PKCE flow and then
refreshes the provider’s standard `/models` roster. Choose the model explicitly;
login never chooses the first model, changes billing groups, or copies another
application’s credentials. A provider selected from `/provider` uses the same
flow instead of asking for an API key.

`/login status` retains account status. `/logout <provider-id>` removes only that
plugin’s local grant. Bare `/logout` retains Codewhale account logout.
Plugin rosters are account-scoped and not reused from the disk cache.
The terminal is temporarily suspended during browser authorization, like the
built-in PKCE flows; device-code and remote revocation are not added here.
A successful authorization followed by a catalog failure retains the grant;
retry catalog refresh rather than copying a token or running a companion login.

## Terminal and noninteractive clients''')
p=r/'docs/features.toml';s=p.read_text().replace('summary = "/login signs in to Codewhale or stores provider keys."','summary = "/login selects providers; /login <id> runs reviewed plugin OAuth; status and account remain explicit."').replace('summary = "Reviewed plugins add OpenAI-compatible routes; the host owns PKCE grants and rechecks approval at each request."','summary = "Reviewed plugins appear in /login and /provider; the host owns PKCE, account-scoped model discovery and per-request approval checks."');s+='''
[[feature]]
id = "plugin-shell-install"
name = "Plugin shell installation"
summary = "codewhale install uses the reviewed bundle installer with local, GitHub and version-pinned npm sources."
status = "experimental"
surfaces = { cli = "preview", tui = "preview" }
since = "unreleased"
docs = "docs/PLUGINS.md"
owner = "crates/tui/src/plugins/mutation.rs"
''';p.write_text(s)
