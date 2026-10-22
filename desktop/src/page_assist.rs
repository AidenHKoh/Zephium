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
        /// The Space the tab was last listed in: where "Run in Work" puts
        /// its Work, and which list can tell that the tab closed.
        space: Option<String>,
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
        profile: Option<(ProfileId, bool, Option<&str>)>,
    ) -> Option<(String, Option<String>)> {
        let viewer = tab.content != zephium_ipc::TabContentView::Web;
        match tabs.get_mut(&tab.id) {
            Some(entry) => {
                if let Some((profile, private, space)) = profile {
                    entry.profile = profile;
                    entry.private = private;
                    entry.space = space.map(str::to_owned);
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
                let (profile, private, space) = profile?;
                if tabs.len() >= MAX_TABS {
                    tabs.clear();
                }
                tabs.insert(
                    tab.id.clone(),
                    TabEntry {
                        profile,
                        space: space.map(str::to_owned),
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
        let Some((profile, private)) = state.profile.as_ref().and_then(|profile| {
            ProfileId::parse(&profile.id)
                .map(|id| (id, profile.kind == zephium_ipc::ProfileKindView::Incognito))
        }) else {
            return;
        };
        let space = state.active_space_id.as_deref();
        let (changes, closed) = with_tabs(|tabs| {
            let changes: Vec<_> = state
                .tabs
                .iter()
                .filter_map(|tab| follow(tabs, tab, Some((profile, private, space))))
                .collect();
            let closed = closed_tabs(tabs, state, profile, space);
            for tab in &closed {
                tabs.remove(tab);
            }
            (changes, closed)
        });
        navigated(changes);
        if let Some(service) = service() {
            for tab in closed {
                service.tab_closed(&tab);
            }
        }
    }

    /// Tabs this profile and Space listed before and no longer list: closed.
    /// A tab in another Space is never judged from this Space's list.
    fn closed_tabs(
        tabs: &HashMap<String, TabEntry>,
        state: &zephium_ipc::ItemsState,
        profile: ProfileId,
        space: Option<&str>,
    ) -> Vec<String> {
        let Some(space) = space else {
            return Vec::new();
        };
        tabs.iter()
            .filter(|(id, entry)| {
                entry.profile == profile
                    && entry.space.as_deref() == Some(space)
                    && !state.tabs.iter().any(|tab| &tab.id == *id)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Space and address of a tab, for "Run in Work".
    fn tab_place(tab: &str) -> Option<String> {
        with_tabs(|tabs| tabs.get(tab).and_then(|entry| entry.space.clone()))
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

    /// "Run in Work": a new Work in the tab's Space with the question as its
    /// objective and the tab on its canvas, run through the ordinary Work
    /// operation owner.
    struct DesktopHandoff(tauri::AppHandle);
    impl WorkHandoff for DesktopHandoff {
        fn begin<'a>(
            &'a self,
            profile: &'a ProfileId,
            tab: &'a str,
            question: String,
            url: String,
        ) -> BoxFuture<'a, Option<String>> {
            Box::pin(async move {
                let space = tab_place(tab)?;
                super::handoff::start(&self.0, *profile, tab, &space, question, url)
                    .await
                    .ok()
            })
        }
    }

    /// Page styles in app settings, one key per profile and site, so no
    /// value outgrows the settings row.
    struct SettingStyles(Arc<zephium_store::SqliteStore>);
    /// The settings store takes values up to 64 KiB; stay clear of it.
    const MAX_SETTING_BYTES: usize = 60 * 1024;

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
        fn save(&self, profile: &ProfileId, styles: &[StoredStyle]) -> bool {
            let old: Vec<String> = self
                .0
                .app_setting(&Self::index_key(profile))
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default();
            let mut sites: Vec<String> = styles.iter().map(|style| style.site.clone()).collect();
            sites.sort();
            sites.dedup();
            let mut rows = Vec::with_capacity(sites.len());
            for site in &sites {
                let site_rows: Vec<Row> = styles
                    .iter()
                    .filter(|style| &style.site == site)
                    .map(|style| Row {
                        id: style.id.clone(),
                        summary: style.summary.clone(),
                        css: style.css.clone(),
                        enabled: style.enabled,
                    })
                    .collect();
                let Ok(text) = serde_json::to_string(&site_rows) else {
                    return false;
                };
                // Refuse before writing anything: no row outgrows a value.
                if text.len() > MAX_SETTING_BYTES {
                    return false;
                }
                rows.push((site, text));
            }
            let Ok(index) = serde_json::to_string(&sites) else {
                return false;
            };
            if index.len() > MAX_SETTING_BYTES {
                return false;
            }
            let mut stored = true;
            for site in old.iter().filter(|site| !sites.contains(site)) {
                stored &= self
                    .0
                    .set_app_setting(Self::site_key(profile, site), "[]".to_owned());
            }
            for (site, text) in rows {
                stored &= self.0.set_app_setting(Self::site_key(profile, site), text);
            }
            stored & self.0.set_app_setting(Self::index_key(profile), index)
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
            work: Arc::new(DesktopHandoff(app.clone())),
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

/// "Run in Work" from the Ask panel: the same Work documents and operation
/// owner the panel's own Work flow uses, in order.
#[cfg(feature = "work-product")]
mod handoff {
    use super::*;
    use std::time::Duration;
    use zephium_core::ids::{ItemId, ProfileId, SpaceId};
    use zephium_core::work::authoring::WorkAuthoringIntent;
    use zephium_core::work::context::{WorkContextSelectionItem, WorkContextSelectionV1};
    use zephium_core::work::environment::{
        WorkEnvironmentCall, WorkEnvironmentEdit, WorkEnvironmentIntent, WorkEnvironmentReference,
        WorkEnvironmentReply, WorkEnvironmentSnapshot,
    };
    use zephium_core::work::runtime::WorkRuntimeIntent;
    use zephium_core::work::{WorkCommandId, WorkError};
    use zephium_ipc::work::{
        WorkAuthoringCommandV1, WorkCallV1, WorkCommandV1, WorkOperationStateV1, WorkOperationV1,
        WorkReplyV1,
    };

    const TIMEOUT: Duration = Duration::from_secs(8);
    const MAX_TITLE_CHARS: usize = 80;

    async fn call(
        app: &tauri::AppHandle,
        profile: ProfileId,
        call: WorkCallV1,
    ) -> Result<WorkReplyV1, WorkError> {
        let request = app
            .state::<zephium_app::Handle>()
            .work_call(profile, call)?;
        let response = tokio::time::timeout(TIMEOUT, request.response(profile))
            .await
            .map_err(|_| WorkError::OutcomeUnknown)?;
        match response.reply {
            WorkReplyV1::Error { .. } => Err(WorkError::Unavailable),
            reply => Ok(reply),
        }
    }

    fn environment(intent: WorkEnvironmentIntent) -> WorkCallV1 {
        WorkCallV1::Environment {
            version: 1,
            request: WorkEnvironmentCall::Command {
                command: WorkCommandId::generate(),
                intent,
            },
        }
    }

    fn added(
        snapshot: &WorkEnvironmentSnapshot,
        reference: WorkEnvironmentReference,
    ) -> WorkEnvironmentIntent {
        WorkEnvironmentIntent::Edit {
            id: snapshot.id,
            expected: snapshot.revision,
            edit: WorkEnvironmentEdit::Add {
                reference,
                area: None,
            },
        }
    }

    fn applied(reply: WorkReplyV1) -> Result<Box<WorkEnvironmentSnapshot>, WorkError> {
        match reply {
            WorkReplyV1::Environment {
                reply: WorkEnvironmentReply::Applied { snapshot, .. },
            } => Ok(snapshot),
            _ => Err(WorkError::Unavailable),
        }
    }

    /// The panel's own routine grant and limits (`AGENT_GRANT`, `AGENT_LIMITS`
    /// in `frame/src/domain/work/work-model.ts`).
    pub(super) fn begin_agent() -> Result<WorkRuntimeIntent, WorkError> {
        serde_json::from_value(serde_json::json!({
            "kind": "begin_agent",
            "grant": {
                "provider": "open_ai",
                "model": "gpt-6-luna",
                "max_turns": 10,
                "max_steps": 32,
                "browse_hops": 4
            },
            "limits": {
                "model_tokens": 1_000_000,
                "cost_micro_usd": 3_000_000,
                "operations": 256,
                "timeout_seconds": 1800,
                "max_workers": 4
            }
        }))
        .map_err(|_| WorkError::Invalid)
    }

    /// Creates a Work in the tab's Space with the tab and the objective on
    /// its canvas, then begins a run with the tab as context.
    pub(super) async fn start(
        app: &tauri::AppHandle,
        profile: ProfileId,
        tab: &str,
        space: &str,
        question: String,
        url: String,
    ) -> Result<String, WorkError> {
        let tab = ItemId::parse(tab).ok_or(WorkError::Invalid)?;
        let space = SpaceId::parse(space).ok_or(WorkError::Invalid)?;
        let intent = begin_agent()?;
        let title = zephium_work_composition::page_assist::core::text::bounded(
            &zephium_work_composition::page_assist::core::text::clean(&question),
            MAX_TITLE_CHARS,
        );
        let created = applied(
            call(
                app,
                profile,
                environment(WorkEnvironmentIntent::Create { space, title }),
            )
            .await?,
        )?;
        let browser = WorkEnvironmentReference::Browser { tab };
        let snapshot =
            applied(call(app, profile, environment(added(&created, browser.clone()))).await?)?;
        let element = snapshot
            .elements
            .iter()
            .find(|element| element.reference == browser)
            .map(|element| element.id)
            .ok_or(WorkError::Unavailable)?;
        let WorkReplyV1::AuthoringApplied { receipt } = call(
            app,
            profile,
            WorkCallV1::Author {
                command: WorkAuthoringCommandV1 {
                    version: 1,
                    command: WorkCommandId::generate(),
                    intent: WorkAuthoringIntent::Create {
                        objective: question,
                    },
                },
            },
        )
        .await?
        else {
            return Err(WorkError::Unavailable);
        };
        // The objective joins the canvas beside its tab; the run does not
        // depend on it, so a refusal here does not stop the run.
        let objective = WorkEnvironmentReference::Objective {
            objective: receipt.work,
        };
        let _ = call(app, profile, environment(added(&snapshot, objective))).await;
        let operation = WorkCommandId::generate();
        let input = WorkOperationV1::Run {
            command: WorkCommandV1 {
                version: 1,
                work: receipt.work,
                expected_revision: receipt.applied_revision,
                command: WorkCommandId::generate(),
                intent,
            },
            context: Some(WorkContextSelectionV1 {
                environment: snapshot.id,
                items: vec![WorkContextSelectionItem {
                    element,
                    revision: url,
                }],
                tabs: false,
                blocks: false,
            }),
            signed_in: Default::default(),
        };
        match super::super::work_product::admit_run(app, profile, operation, input)? {
            WorkOperationStateV1::Refused { .. } => Err(WorkError::Unavailable),
            _ => Ok(receipt.work.to_string()),
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn the_grant_matches_the_panels_own() {
            assert!(super::begin_agent().is_ok());
            let frame = include_str!("../../frame/src/domain/work/work-model.ts");
            for pinned in [
                "provider: \"open_ai\"",
                "model: \"gpt-6-luna\"",
                "max_turns: 10",
                "max_steps: 32",
                "browse_hops: 4",
                "model_tokens: 1_000_000",
                "cost_micro_usd: 3_000_000",
                "operations: 256",
                "timeout_seconds: 1800",
                "max_workers: 4",
            ] {
                assert!(
                    frame.contains(pinned),
                    "{pinned} drifted from the panel's grant"
                );
            }
        }
    }
}
