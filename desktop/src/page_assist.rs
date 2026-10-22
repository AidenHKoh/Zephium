//! The Ask panel's commands and its one event. Admission and gating live
//! here; the thread, the page read and the model call belong to the
//! page-assist service in `zephium-work-composition`.
use super::*;
use zephium_ipc::page_assist::*;

/// One change to a tab's Ask thread.
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "zephium:page-assist-changed")]
pub(crate) struct PageAssistChanged(pub(crate) PageAssistChangedV1);

const VERSION: u16 = 1;
const EVENT: &str = "zephium:page-assist-changed";

fn allowed(caller: &WebviewWindow, command: &'static str) -> bool {
    authorize(caller, CallerPolicy::Main, command)
}

fn unavailable_status(tab: String) -> PageAssistStatusV1 {
    PageAssistStatusV1 {
        version: VERSION,
        tab,
        availability: PageAssistAvailabilityV1::Unavailable,
        site: None,
        model: None,
        suggestions: Vec::new(),
        selection: None,
        can_act: false,
        styled: false,
        entries: Vec::new(),
    }
}

fn no_styles(tab: String) -> PageStylesV1 {
    PageStylesV1 {
        version: VERSION,
        tab,
        site: None,
        styles: Vec::new(),
    }
}

fn no_profile_styles(profile: String) -> PageStylesAllV1 {
    PageStylesAllV1 {
        version: VERSION,
        profile,
        sites: Vec::new(),
    }
}

fn valid_tab(tab: &str) -> bool {
    (1..=64).contains(&tab.len())
        && tab
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
}

#[cfg(feature = "work-product")]
mod product {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::Instant;
    use zephium_core::ids::ProfileId;
    use zephium_core::ports::engine::Engine as _;
    use zephium_core::ports::store::Store as _;
    use zephium_core::work::model::{WorkModelEntry, WorkModelError, WorkModelRole};
    use zephium_work_composition::page_assist::port::*;
    use zephium_work_composition::page_assist::{PageAssist, Ports};

    static SERVICE: OnceLock<PageAssist> = OnceLock::new();

    pub(super) fn service() -> Option<&'static PageAssist> {
        SERVICE.get()
    }

    #[derive(Clone)]
    struct TabEntry {
        profile: ProfileId,
        private: bool,
        url: Option<String>,
        loading: bool,
        viewer: bool,
        epoch: u64,
    }

    static TABS: Mutex<Option<HashMap<String, TabEntry>>> = Mutex::new(None);
    const MAX_TABS: usize = 4096;

    fn with_tabs<T>(apply: impl FnOnce(&mut HashMap<String, TabEntry>) -> T) -> T {
        let mut tabs = TABS.lock().unwrap_or_else(|p| p.into_inner());
        apply(tabs.get_or_insert_with(HashMap::new))
    }

    /// Follows one tab's committed address; a change is a navigation.
    fn follow(
        tabs: &mut HashMap<String, TabEntry>,
        tab: &zephium_ipc::TabView,
        profile: Option<(ProfileId, bool)>,
    ) -> Option<(String, Option<String>)> {
        let viewer = tab.content != zephium_ipc::TabContentView::Web;
        match tabs.get_mut(&tab.id) {
            Some(entry) => {
                if let Some((profile, private)) = profile {
                    entry.profile = profile;
                    entry.private = private;
                }
                entry.loading = tab.loading;
                entry.viewer = viewer;
                if entry.url != tab.url {
                    entry.url = tab.url.clone();
                    entry.epoch = entry.epoch.wrapping_add(1);
                    return Some((tab.id.clone(), tab.url.clone()));
                }
                None
            }
            None => {
                let (profile, private) = profile?;
                if tabs.len() >= MAX_TABS {
                    tabs.clear();
                }
                tabs.insert(
                    tab.id.clone(),
                    TabEntry {
                        profile,
                        private,
                        url: tab.url.clone(),
                        loading: tab.loading,
                        viewer,
                        epoch: 1,
                    },
                );
                None
            }
        }
    }

    fn navigated(changes: Vec<(String, Option<String>)>) {
        let Some(service) = service() else {
            return;
        };
        for (tab, url) in changes {
            service.navigated(&tab, url.as_deref());
        }
    }

    pub(crate) fn observe_items(state: &zephium_ipc::ItemsState) {
        let Some(profile) = state.profile.as_ref().and_then(|profile| {
            ProfileId::parse(&profile.id)
                .map(|id| (id, profile.kind == zephium_ipc::ProfileKindView::Incognito))
        }) else {
            return;
        };
        let changes = with_tabs(|tabs| {
            state
                .tabs
                .iter()
                .filter_map(|tab| follow(tabs, tab, Some(profile)))
                .collect()
        });
        navigated(changes);
    }

    pub(crate) fn observe_tab(tab: &zephium_ipc::TabView) {
        let change = with_tabs(|tabs| follow(tabs, tab, None));
        navigated(change.into_iter().collect());
    }

    pub(crate) fn preference_changed(key: &str, value: &str) {
        if key == "ai.enabled" && value == "false" {
            if let Some(service) = service() {
                service.end_all();
            }
        }
    }

    /// The live tab. Reading and steps wait on the Browse reader, so they
    /// report this platform as unsupported; page styles already apply.
    struct DesktopPage {
        engine: Arc<zephium_engine::WebviewEngine>,
    }

    impl PagePort for DesktopPage {
        fn supported(&self) -> bool {
            false
        }
        fn facts(&self, tab: &str) -> Option<TabFacts> {
            with_tabs(|tabs| {
                tabs.get(tab).map(|entry| TabFacts {
                    profile: entry.profile,
                    url: entry.url.clone(),
                    private: entry.private,
                    viewer: entry.viewer,
                    loading: entry.loading,
                    epoch: entry.epoch,
                })
            })
        }
        fn read<'a>(
            &'a self,
            _tab: &'a str,
            _epoch: u64,
        ) -> BoxFuture<'a, Result<Readout, ReadFailure>> {
            Box::pin(async { Err(ReadFailure::Unavailable) })
        }
        fn selection<'a>(&'a self, _tab: &'a str, _epoch: u64) -> BoxFuture<'a, Option<String>> {
            Box::pin(async { None })
        }
        fn highlight(&self, _tab: &str, _epoch: u64, _text: &str) -> bool {
            false
        }
        fn resolve(
            &self,
            _tab: &str,
            _epoch: u64,
            _step: &StepRequest,
        ) -> Result<ResolvedStep, StepFailure> {
            Err(StepFailure::Refused)
        }
        fn act<'a>(
            &'a self,
            _tab: &'a str,
            _epoch: u64,
            _step: &'a StepRequest,
        ) -> BoxFuture<'a, Result<(), StepFailure>> {
            Box::pin(async { Err(StepFailure::Refused) })
        }
        fn apply_styles(&self, profile: &ProfileId, site: &str, css: &str) {
            self.engine
                .set_page_style(*profile, site.to_owned(), Arc::from(css));
        }
    }

    struct Models;

    impl ModelSource for Models {
        fn entry<'a>(&'a self, profile: &'a ProfileId) -> BoxFuture<'a, Option<WorkModelEntry>> {
            Box::pin(async move {
                zephium_app::work_models::resolve_entry_for(*profile, WorkModelRole::Page, None)
                    .await
                    .ok()
                    .map(|(entry, _)| entry)
            })
        }
        fn resolve<'a>(
            &'a self,
            profile: &'a ProfileId,
            source: zephium_app::work_models::WorkCallSource,
        ) -> BoxFuture<'a, Result<ResolvedModel, WorkModelError>> {
            Box::pin(async move {
                let (entry, client) = zephium_app::work_models::resolve_entry_for(
                    *profile,
                    WorkModelRole::Page,
                    Some(source),
                )
                .await?;
                Ok(ResolvedModel { entry, client })
            })
        }
    }

    struct Events(tauri::AppHandle);
    impl EventSink for Events {
        fn emit(&self, event: PageAssistChangedV1) {
            emit_to_privileged(&self.0, MAIN_LABEL, EVENT, &event);
        }
    }

    struct Monotonic(Instant);
    impl Clock for Monotonic {
        fn now_ms(&self) -> u64 {
            u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
        }
    }

    struct Switches(tauri::AppHandle);
    impl Settings for Switches {
        fn ai_enabled(&self, _profile: &ProfileId) -> bool {
            self.0
                .try_state::<keymap::Keymap>()
                .is_some_and(|keymap| keymap.ai_enabled())
        }
    }

    /// Work starts from the panel's own Work bar for now; this path reports
    /// it unavailable rather than half-starting a run.
    struct NoHandoff;
    impl WorkHandoff for NoHandoff {
        fn begin<'a>(
            &'a self,
            _profile: &'a ProfileId,
            _question: String,
            _url: String,
        ) -> BoxFuture<'a, Option<String>> {
            Box::pin(async { None })
        }
    }

    /// Page styles in app settings, one key per profile and site, so no
    /// value outgrows the settings row.
    struct SettingStyles(Arc<zephium_store::SqliteStore>);

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Row {
        id: String,
        summary: String,
        css: String,
        enabled: bool,
    }

    impl SettingStyles {
        fn index_key(profile: &ProfileId) -> String {
            format!("page_assist.sites.{profile}")
        }
        fn site_key(profile: &ProfileId, site: &str) -> String {
            format!("page_assist.style.{profile}.{site}")
        }
    }

    impl StyleStore for SettingStyles {
        fn load(&self, profile: &ProfileId) -> Vec<StoredStyle> {
            let sites: Vec<String> = self
                .0
                .app_setting(&Self::index_key(profile))
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default();
            let mut styles = Vec::new();
            for site in sites.into_iter().take(512) {
                let rows: Vec<Row> = self
                    .0
                    .app_setting(&Self::site_key(profile, &site))
                    .and_then(|text| serde_json::from_str(&text).ok())
                    .unwrap_or_default();
                for row in rows {
                    // Stored CSS passes the validator again before it is used.
                    let Ok(valid) =
                        zephium_work_composition::page_assist::core::css::validate(&row.css)
                    else {
                        continue;
                    };
                    styles.push(StoredStyle {
                        id: row.id,
                        site: site.clone(),
                        summary: row.summary,
                        css: valid.css,
                        enabled: row.enabled,
                    });
                }
            }
            styles
        }
        fn save(&self, profile: &ProfileId, styles: &[StoredStyle]) {
            let old: Vec<String> = self
                .0
                .app_setting(&Self::index_key(profile))
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default();
            let mut sites: Vec<String> = styles.iter().map(|style| style.site.clone()).collect();
            sites.sort();
            sites.dedup();
            for site in old.iter().filter(|site| !sites.contains(site)) {
                self.0
                    .set_app_setting(Self::site_key(profile, site), "[]".to_owned());
            }
            for site in &sites {
                let rows: Vec<Row> = styles
                    .iter()
                    .filter(|style| &style.site == site)
                    .map(|style| Row {
                        id: style.id.clone(),
                        summary: style.summary.clone(),
                        css: style.css.clone(),
                        enabled: style.enabled,
                    })
                    .collect();
                if let Ok(text) = serde_json::to_string(&rows) {
                    self.0.set_app_setting(Self::site_key(profile, site), text);
                }
            }
            if let Ok(text) = serde_json::to_string(&sites) {
                self.0.set_app_setting(Self::index_key(profile), text);
            }
        }
    }

    pub(crate) fn install(
        app: &tauri::AppHandle,
        engine: Arc<zephium_engine::WebviewEngine>,
        store: Arc<zephium_store::SqliteStore>,
    ) {
        let service = PageAssist::new(Ports {
            page: Arc::new(DesktopPage { engine }),
            models: Arc::new(Models),
            events: Arc::new(Events(app.clone())),
            clock: Arc::new(Monotonic(Instant::now())),
            settings: Arc::new(Switches(app.clone())),
            work: Arc::new(NoHandoff),
            styles: Arc::new(SettingStyles(store)),
        });
        let _ = SERVICE.set(service);
    }

    /// Hosted requests of Ask turns, until their charge is known.
    static HOSTED: Mutex<Vec<(String, String, u32)>> = Mutex::new(Vec::new());

    pub(crate) fn hosted_sourced(request_id: &str, tab: &str, turn: u32) {
        let mut hosted = HOSTED.lock().unwrap_or_else(|p| p.into_inner());
        if hosted.len() >= 64 {
            hosted.remove(0);
        }
        hosted.push((request_id.to_owned(), tab.to_owned(), turn));
    }

    /// A hosted request's charge is final: show it under its answer.
    pub(crate) fn hosted_charged(
        request_id: &str,
        charged_units: Option<u64>,
        units_per_credit: Option<u64>,
    ) {
        let entry = {
            let mut hosted = HOSTED.lock().unwrap_or_else(|p| p.into_inner());
            let Some(index) = hosted.iter().position(|(id, ..)| id == request_id) else {
                return;
            };
            hosted.remove(index)
        };
        let (Some(charged), Some(per), Some(service)) =
            (charged_units, units_per_credit, service())
        else {
            return;
        };
        if let Some(credits) =
            zephium_work_composition::page_assist::core::cost::credits(charged, per)
        {
            service.settle_hosted(&entry.1, entry.2, credits);
        }
    }
}

#[cfg(feature = "work-product")]
pub(crate) use product::{
    hosted_charged, hosted_sourced, install, observe_items, observe_tab, preference_changed,
};

#[cfg(not(feature = "work-product"))]
pub(crate) fn observe_items(_state: &zephium_ipc::ItemsState) {}
#[cfg(not(feature = "work-product"))]
pub(crate) fn observe_tab(_tab: &zephium_ipc::TabView) {}
#[cfg(not(feature = "work-product"))]
pub(crate) fn preference_changed(_key: &str, _value: &str) {}

macro_rules! with_service {
    ($none:expr, |$service:ident| $body:expr) => {{
        #[cfg(feature = "work-product")]
        {
            match product::service() {
                Some($service) => $body,
                None => $none,
            }
        }
        #[cfg(not(feature = "work-product"))]
        {
            $none
        }
    }};
}

/// The tab's availability, suggestions and whole thread.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_status(caller: WebviewWindow, tab: String) -> PageAssistStatusV1 {
    if !allowed(&caller, "page_assist_status") || !valid_tab(&tab) {
        return unavailable_status(tab);
    }
    with_service!(unavailable_status(tab), |service| service
        .status(&tab)
        .await)
}

/// Starts a turn. Admission only; the answer arrives as events.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_ask(
    caller: WebviewWindow,
    request: PageAssistAskV1,
) -> PageAssistAskResponseV1 {
    let refused = |fault| PageAssistAskResponseV1 {
        turn: None,
        fault: Some(fault),
    };
    if !allowed(&caller, "page_assist_ask") {
        return refused(PageAssistFaultV1::Unavailable);
    }
    if !valid_tab(&request.tab) || request.text.len() > PAGE_ASSIST_MAX_QUESTION_BYTES {
        return refused(PageAssistFaultV1::Invalid);
    }
    with_service!(refused(PageAssistFaultV1::Unavailable), |service| {
        match service.ask(request).await {
            Ok((turn, task)) => {
                tauri::async_runtime::spawn(task);
                PageAssistAskResponseV1 {
                    turn: Some(turn),
                    fault: None,
                }
            }
            Err(fault) => refused(fault),
        }
    })
}

/// Stops the tab's running turn; nothing more is sent or applied.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_stop(caller: WebviewWindow, tab: String) -> bool {
    allowed(&caller, "page_assist_stop") && with_service!(false, |service| service.stop(&tab))
}

/// Scrolls to and highlights citation `n` of `turn` in the page.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_cite(caller: WebviewWindow, tab: String, turn: u32, n: u8) -> bool {
    allowed(&caller, "page_assist_cite")
        && with_service!(false, |service| service.cite(&tab, turn, n))
}

/// Answers the turn's waiting confirm or style Apply.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_decide(
    caller: WebviewWindow,
    tab: String,
    turn: u32,
    decision: PageAssistDecisionV1,
) -> bool {
    allowed(&caller, "page_assist_decide")
        && with_service!(false, |service| service.decide(&tab, turn, decision))
}

/// Continues a paused turn after the person used the page; it reads again.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_resume(caller: WebviewWindow, tab: String, turn: u32) -> bool {
    allowed(&caller, "page_assist_resume")
        && with_service!(false, |service| service.resume(&tab, turn))
}

/// Ends the tab's thread.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_clear(caller: WebviewWindow, tab: String) -> bool {
    allowed(&caller, "page_assist_clear") && with_service!(false, |service| service.clear(&tab))
}

/// Starts a Work run with the turn's question and this tab.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_handoff(
    caller: WebviewWindow,
    tab: String,
    turn: u32,
) -> PageAssistHandoffV1 {
    let unavailable = PageAssistHandoffV1 {
        work: None,
        fault: Some(PageAssistFaultV1::Unavailable),
    };
    if !allowed(&caller, "page_assist_handoff") {
        return unavailable;
    }
    with_service!(unavailable, |service| service.handoff(&tab, turn).await)
}

/// The page styles of the site in `tab`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_styles(caller: WebviewWindow, tab: String) -> PageStylesV1 {
    if !allowed(&caller, "page_styles") {
        return no_styles(tab);
    }
    with_service!(no_styles(tab), |service| service.styles(&tab))
}

/// Turns one of the site's styles on or off.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_style_toggle(
    caller: WebviewWindow,
    tab: String,
    style: String,
    enabled: bool,
) -> PageStylesV1 {
    if !allowed(&caller, "page_style_toggle") || !valid_id(&style) {
        return no_styles(tab);
    }
    with_service!(no_styles(tab), |service| service
        .toggle_style(&tab, &style, enabled))
}

/// Deletes one of the site's styles.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_style_remove(
    caller: WebviewWindow,
    tab: String,
    style: String,
) -> PageStylesV1 {
    if !allowed(&caller, "page_style_remove") || !valid_id(&style) {
        return no_styles(tab);
    }
    with_service!(no_styles(tab), |service| service.remove_style(&tab, &style))
}

#[cfg(feature = "work-product")]
fn profile_of(profile: &str) -> Option<zephium_core::ids::ProfileId> {
    zephium_core::ids::ProfileId::parse(profile).filter(|id| id.to_string() == profile)
}

/// Every site's page styles in `profile`, for Settings.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_styles_all(caller: WebviewWindow, profile: String) -> PageStylesAllV1 {
    if !allowed(&caller, "page_styles_all") {
        return no_profile_styles(profile);
    }
    with_service!(no_profile_styles(profile), |service| {
        match profile_of(&profile) {
            Some(id) => service.all_styles(&id),
            None => no_profile_styles(profile),
        }
    })
}

/// Turns one of a site's styles on or off from Settings.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_style_toggle_for(
    caller: WebviewWindow,
    profile: String,
    site: String,
    style: String,
    enabled: bool,
) -> PageStylesAllV1 {
    if !allowed(&caller, "page_style_toggle_for") || !valid_id(&style) || site.len() > 253 {
        return no_profile_styles(profile);
    }
    with_service!(no_profile_styles(profile), |service| {
        match profile_of(&profile) {
            Some(id) => service.toggle_style_for(&id, &site, &style, enabled),
            None => no_profile_styles(profile),
        }
    })
}

/// Deletes one of a site's styles from Settings.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_style_remove_for(
    caller: WebviewWindow,
    profile: String,
    site: String,
    style: String,
) -> PageStylesAllV1 {
    if !allowed(&caller, "page_style_remove_for") || !valid_id(&style) || site.len() > 253 {
        return no_profile_styles(profile);
    }
    with_service!(no_profile_styles(profile), |service| {
        match profile_of(&profile) {
            Some(id) => service.remove_style_for(&id, &site, &style),
            None => no_profile_styles(profile),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_and_style_ids_are_closed() {
        assert!(valid_tab("01HZX3J5Q4F0Y8N9W2E6R7T1M3"));
        for bad in ["", "a b", "../x", &"a".repeat(65)] {
            assert!(!valid_tab(bad), "{bad}");
        }
        assert!(valid_id("s123-4"));
        assert!(!valid_id("s1\"};"));
    }
}
