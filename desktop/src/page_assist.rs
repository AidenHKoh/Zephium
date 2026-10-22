//! The Ask panel's commands and its one event. Admission, gating and fault
//! mapping live here; the thread, the page read and the model call belong to
//! the page-assist service.
use super::*;
use zephium_ipc::page_assist::*;

/// One change to a tab's Ask thread.
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "zephium:page-assist-changed")]
pub(crate) struct PageAssistChanged(pub(crate) PageAssistChangedV1);

const VERSION: u16 = 1;

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

/// The tab's availability, suggestions and whole thread.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_status(caller: WebviewWindow, tab: String) -> PageAssistStatusV1 {
    let _ = allowed(&caller, "page_assist_status");
    unavailable_status(tab)
}

/// Starts a turn. Admission only; the answer arrives as events.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_ask(
    caller: WebviewWindow,
    request: PageAssistAskV1,
) -> PageAssistAskResponseV1 {
    let fault = if !allowed(&caller, "page_assist_ask") {
        PageAssistFaultV1::Unavailable
    } else if request.text.len() > PAGE_ASSIST_MAX_QUESTION_BYTES
        || (request.text.trim().is_empty() && request.suggestion.is_none())
    {
        PageAssistFaultV1::Invalid
    } else {
        PageAssistFaultV1::Unavailable
    };
    PageAssistAskResponseV1 {
        turn: None,
        fault: Some(fault),
    }
}

/// Stops the tab's running turn; nothing more is sent or applied.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_stop(caller: WebviewWindow, tab: String) -> bool {
    let _ = (allowed(&caller, "page_assist_stop"), tab);
    false
}

/// Scrolls to and highlights citation `n` of `turn` in the page.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_cite(caller: WebviewWindow, tab: String, turn: u32, n: u8) -> bool {
    let _ = (allowed(&caller, "page_assist_cite"), tab, turn, n);
    false
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
    let _ = (allowed(&caller, "page_assist_decide"), tab, turn, decision);
    false
}

/// Continues a paused turn after the person used the page; it reads again.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_resume(caller: WebviewWindow, tab: String, turn: u32) -> bool {
    let _ = (allowed(&caller, "page_assist_resume"), tab, turn);
    false
}

/// Ends the tab's thread.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_clear(caller: WebviewWindow, tab: String) -> bool {
    let _ = (allowed(&caller, "page_assist_clear"), tab);
    false
}

/// Starts a Work run with the turn's question and this tab.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_assist_handoff(
    caller: WebviewWindow,
    tab: String,
    turn: u32,
) -> PageAssistHandoffV1 {
    let _ = (allowed(&caller, "page_assist_handoff"), tab, turn);
    PageAssistHandoffV1 {
        work: None,
        fault: Some(PageAssistFaultV1::Unavailable),
    }
}

/// The page styles of the site in `tab`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_styles(caller: WebviewWindow, tab: String) -> PageStylesV1 {
    let _ = allowed(&caller, "page_styles");
    no_styles(tab)
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
    let _ = (allowed(&caller, "page_style_toggle"), style, enabled);
    no_styles(tab)
}

/// Deletes one of the site's styles.
#[tauri::command]
#[specta::specta]
pub(crate) async fn page_style_remove(
    caller: WebviewWindow,
    tab: String,
    style: String,
) -> PageStylesV1 {
    let _ = (allowed(&caller, "page_style_remove"), style);
    no_styles(tab)
}
