//! A page agent working on one site in the person's session: it navigates,
//! searches, filters, opens items and fills drafts. Rust classifies every
//! proposed step from the observed page. A step that would commit something
//! (send, post, pay, book, delete, share, save, submit) stops for the person
//! with a preview Rust builds from the page; only an approved step whose
//! target and preview are unchanged may then run, once. Credentials are
//! never typed.
use super::*;
use sha2::{Digest, Sha256};
use std::sync::Mutex;

/// What a held step commits; the person's card names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Consequence {
    Communication,
    Purchase,
    Destructive,
    Save,
    /// Typing into a document that saves as it is typed.
    Edit,
    /// Typing into any field of a site the person did not name, in a run
    /// that holds their own data: the text could carry it to that site.
    Type,
}

impl Consequence {
    pub(crate) const fn class(self) -> SemanticEffectClass {
        match self {
            Self::Communication => SemanticEffectClass::Communication,
            Self::Purchase => SemanticEffectClass::Purchase,
            Self::Destructive => SemanticEffectClass::Destructive,
            Self::Save | Self::Edit => SemanticEffectClass::ExternalWrite,
            Self::Type => SemanticEffectClass::LocalWrite,
        }
    }
    const fn declared(class: SemanticEffectClass) -> Option<Self> {
        match class {
            SemanticEffectClass::Communication => Some(Self::Communication),
            SemanticEffectClass::Purchase => Some(Self::Purchase),
            SemanticEffectClass::Destructive => Some(Self::Destructive),
            SemanticEffectClass::ExternalWrite => Some(Self::Save),
            _ => None,
        }
    }
}

/// What one proposed step would do, read from the page, never from the model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SiteEffect {
    Read,
    Draft,
    Commit(Consequence),
}

/// A cart is the person's to change and empty again: putting something in
/// it saves, it buys nothing. Placing the order or paying is the purchase.
const CART_WORDS: [&str; 12] = [
    "add to cart",
    "add to basket",
    "add to bag",
    "add to trolley",
    "update cart",
    "dodaj do koszyka",
    "do koszyka",
    "in den warenkorb",
    "ajouter au panier",
    "añadir al carrito",
    "aggiungi al carrello",
    "add to wishlist",
];
/// Words on a control or its region, by what they commit. Earlier lists win.
const PURCHASE_WORDS: [&str; 19] = [
    "buy",
    "purchase",
    "pay",
    "order",
    "book",
    "reserve",
    "subscribe",
    "donate",
    "bid",
    "request to book",
    "place order",
    "book now",
    "reserve now",
    "confirm and pay",
    "kup",
    "zapłać",
    "zamów",
    "zarezerwuj",
    "kupuję",
];
const DESTRUCTIVE_WORDS: [&str; 21] = [
    "delete",
    "remove",
    "discard",
    "archive",
    "trash",
    "erase",
    "unsubscribe",
    "leave",
    "deactivate",
    "disconnect",
    "revoke",
    "cancel subscription",
    "cancel order",
    "cancel booking",
    "cancel reservation",
    "cancel plan",
    "delete forever",
    "usuń",
    "anuluj subskrypcję",
    "opuść",
    "wyrzuć",
];
const COMMUNICATION_WORDS: [&str; 22] = [
    "send",
    "post",
    "publish",
    "reply",
    "comment",
    "share",
    "invite",
    "tweet",
    "join",
    "follow",
    "like",
    "react",
    "accept",
    "decline",
    "approve",
    "reject",
    "rsvp",
    "message",
    "wyślij",
    "opublikuj",
    "odpowiedz",
    "udostępnij",
];
const SAVE_WORDS: [&str; 16] = [
    "submit",
    "save",
    "update",
    "confirm",
    "transfer",
    "apply changes",
    "sign up",
    "change password",
    "change email",
    "sign out",
    "log out",
    "logout",
    "signout",
    "zapisz",
    "potwierdź",
    "wyloguj",
];
/// Controls that close or step back and so commit nothing.
const DISMISS_WORDS: [&str; 13] = [
    "close",
    "dismiss",
    "cancel",
    "not now",
    "back",
    "later",
    "skip",
    "no thanks",
    "zamknij",
    "nie teraz",
    "schließen",
    "fermer",
    "cerrar",
];
const SIGN_IN_PHRASES: [&str; 8] = [
    "sign in",
    "log in",
    "login",
    "signin",
    "continue with google",
    "continue with apple",
    "zaloguj",
    "sign in with",
];
const CONFIRM_CONTEXT: [&str; 5] = [
    "are you sure",
    "cannot be undone",
    "confirm",
    "permanently",
    "czy na pewno",
];

fn words(text: &str) -> String {
    let words: Vec<_> = text
        .to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect();
    format!(" {} ", words.join(" "))
}

fn names_any(text: &str, phrases: &[&str]) -> bool {
    let text = words(text);
    phrases
        .iter()
        .any(|phrase| text.contains(&format!(" {} ", words(phrase).trim())))
}

fn label(node: &SemanticNode) -> String {
    let mut label = String::new();
    for part in node.name().into_iter().chain(node.text()) {
        label.push(' ');
        label.push_str(part.as_str());
    }
    label
}

fn ancestors<'a>(
    node: &SemanticNode,
    snapshot: &'a SemanticSnapshot,
) -> impl Iterator<Item = (usize, &'a SemanticNode)> {
    let mut parent = node.parent();
    std::iter::from_fn(move || {
        let index = usize::from(parent?);
        let ancestor = snapshot.nodes().get(index)?;
        parent = ancestor.parent();
        Some((index, ancestor))
    })
}

/// The nodes of the subtree rooted at `index`.
fn subtree(snapshot: &SemanticSnapshot, index: usize) -> &[SemanticNode] {
    let Some(root) = snapshot.nodes().get(index) else {
        return &[];
    };
    let end = snapshot
        .nodes()
        .iter()
        .enumerate()
        .skip(index + 1)
        .find(|(_, next)| next.depth() <= root.depth())
        .map_or(snapshot.nodes().len(), |(end, _)| end);
    &snapshot.nodes()[index..end]
}

/// Words in a status line that say something was just committed.
const DONE_WORDS: [&str; 18] = [
    "sent",
    "posted",
    "published",
    "booked",
    "ordered",
    "purchased",
    "paid",
    "deleted",
    "removed",
    "saved",
    "submitted",
    "confirmed",
    "reserved",
    "thank you for your order",
    "wysłano",
    "zapisano",
    "usunięto",
    "opublikowano",
];

fn commit_words(text: &str) -> Option<Consequence> {
    [
        (&CART_WORDS[..], Consequence::Save),
        (&PURCHASE_WORDS[..], Consequence::Purchase),
        (&DESTRUCTIVE_WORDS[..], Consequence::Destructive),
        (&COMMUNICATION_WORDS[..], Consequence::Communication),
        (&SAVE_WORDS[..], Consequence::Save),
    ]
    .into_iter()
    .find(|(words, _)| names_any(text, words))
    .map(|(_, consequence)| consequence)
}

/// A form the browser reports as a search, or a same-origin GET form: its
/// submission is a query, not a commit.
fn queries(node: &SemanticNode) -> bool {
    node.form().is_some_and(|form| {
        form.search() || (form.method() == SemanticFormMethod::Get && form.same_origin())
    })
}

/// Names a site gives its search field when it marks it as no search box
/// (x-kom's "Czego szukasz?").
const SEARCH_NAMES: [&str; 14] = [
    "search",
    "what are you looking for",
    "szukaj",
    "wyszukaj",
    "czego szukasz",
    "suche",
    "suchen",
    "wonach suchst du",
    "rechercher",
    "recherche",
    "buscar",
    "cerca",
    "zoeken",
    "hledat",
];

fn in_search(node: &SemanticNode, snapshot: &SemanticSnapshot) -> bool {
    node.role() == SemanticRole::Searchbox
        || (matches!(node.role(), SemanticRole::Textbox | SemanticRole::Combobox)
            && names_any(&label(node), &SEARCH_NAMES))
        || node.form().is_some_and(|form| form.search())
        || ancestors(node, snapshot).any(|(index, ancestor)| {
            ancestor.landmark_kind() == Some(SemanticLandmarkKind::Search)
                || (ancestor.landmark_kind() == Some(SemanticLandmarkKind::Form)
                    && subtree(snapshot, index)
                        .iter()
                        .any(|part| part.role() == SemanticRole::Searchbox))
        })
}

/// Words that name the thing a control belongs to, not a value inside it:
/// removing one of these removes something the page keeps.
const WHOLE_ITEM: [&str; 24] = [
    "draft",
    "message",
    "email",
    "mail",
    "task",
    "issue",
    "post",
    "comment",
    "page",
    "note",
    "item",
    "event",
    "file",
    "document",
    "reply",
    "thread",
    "conversation",
    "account",
    "project",
    "list",
    "cart",
    "basket",
    "bag",
    "wishlist",
];

/// A remove control inside an open editor that holds unsaved input ("Remove
/// date" on a task being added) takes a value out of the draft: nothing the
/// page keeps changes until the editor's own save. Taking out the whole item
/// ("Discard draft", "Delete task") or anything outside an editor is not that.
fn edits_unsaved_input(text: &str, node: &SemanticNode, snapshot: &SemanticSnapshot) -> bool {
    let index = region_index(node, snapshot);
    let Some(root) = snapshot.nodes().get(index) else {
        return false;
    };
    if root.role() != SemanticRole::Dialog
        && root.landmark_kind() != Some(SemanticLandmarkKind::Form)
    {
        return false;
    }
    let object = words(text);
    let object = DESTRUCTIVE_WORDS.iter().fold(object, |left, word| {
        left.replace(&format!(" {} ", words(word).trim()), " ")
    });
    if object.trim().is_empty() || names_any(&object, &WHOLE_ITEM) {
        return false;
    }
    let region = subtree(snapshot, index);
    region.iter().any(|part| {
        !std::ptr::eq(part, node)
            && (part.role() == SemanticRole::Textbox
                || part.role() == SemanticRole::Combobox
                || part.editable_structure().is_some())
            && value_text(part).is_some()
    }) && region.iter().any(|part| {
        part.role() == SemanticRole::Button && !std::ptr::eq(part, node) && creates(&label(part))
    })
}

/// The region a step acts in: its dialog, form or named region, else the page.
fn region_index(node: &SemanticNode, snapshot: &SemanticSnapshot) -> usize {
    ancestors(node, snapshot)
        .find(|(_, ancestor)| {
            ancestor.role() == SemanticRole::Dialog
                || matches!(
                    ancestor.landmark_kind(),
                    Some(SemanticLandmarkKind::Form | SemanticLandmarkKind::Region)
                )
        })
        .or_else(|| {
            ancestors(node, snapshot)
                .find(|(_, ancestor)| ancestor.landmark_kind() == Some(SemanticLandmarkKind::Main))
        })
        .map_or(0, |(index, _)| index)
}

/// A message box ("Message to design", "Reply…", "Write a comment"): it
/// sends on Enter or its button, never as it is typed.
fn composer(node: &SemanticNode) -> bool {
    names_any(
        &label(node),
        &[
            "message",
            "reply",
            "comment",
            "write a",
            "chat",
            "napisz",
            "wiadomość",
        ],
    )
}

/// A dialog the person drafts something in (a new issue, a new page): it
/// holds a text field, and its own button creates what was drafted.
fn draft_dialog(node: &SemanticNode, snapshot: &SemanticSnapshot) -> Option<usize> {
    let (index, _) =
        ancestors(node, snapshot).find(|(_, ancestor)| ancestor.role() == SemanticRole::Dialog)?;
    let region = subtree(snapshot, index);
    let drafts = region
        .iter()
        .any(|part| matches!(part.role(), SemanticRole::Textbox | SemanticRole::Combobox));
    let creates = region
        .iter()
        .any(|part| part.role() == SemanticRole::Button && creates(&label(part)));
    (drafts && creates).then_some(index)
}

/// The button that creates what this field drafts: in its dialog, or in an
/// editor set in the page (an inline "add task" row), a small part of the
/// page around the field. The page itself never counts: a page of blocks
/// with a "New page" button elsewhere still saves as it is typed.
fn draft_creator<'a>(
    node: &SemanticNode,
    snapshot: &'a SemanticSnapshot,
) -> Option<&'a SemanticNode> {
    const SMALL: usize = 80;
    let creator = |index: usize| {
        subtree(snapshot, index).iter().find(|part| {
            part.role() == SemanticRole::Button
                && !std::ptr::eq(*part, node)
                && creates(&label(part))
        })
    };
    if let Some(index) = draft_dialog(node, snapshot) {
        return creator(index);
    }
    ancestors(node, snapshot)
        .take_while(|(_, ancestor)| ancestor.parent().is_some())
        .take_while(|(index, _)| subtree(snapshot, *index).len() <= SMALL)
        .find_map(|(index, _)| creator(index))
}

/// "Create issue", "Add task", "Save page": the control that makes it.
fn creates(text: &str) -> bool {
    let text = words(text);
    ["create", "add", "save", "submit", "publish", "post", "send"]
        .iter()
        .any(|word| text.starts_with(&format!(" {word} ")))
}

fn dialog_commits(node: &SemanticNode, snapshot: &SemanticSnapshot) -> Option<Consequence> {
    let (index, _) =
        ancestors(node, snapshot).find(|(_, ancestor)| ancestor.role() == SemanticRole::Dialog)?;
    let region = subtree(snapshot, index);
    region
        .iter()
        .any(|part| names_any(&label(part), &CONFIRM_CONTEXT))
        .then(|| {
            region
                .iter()
                .find_map(|part| commit_words(&label(part)))
                .unwrap_or(Consequence::Save)
        })
}

fn pays(node: &SemanticNode, snapshot: &SemanticSnapshot) -> bool {
    subtree(snapshot, region_index(node, snapshot))
        .iter()
        .any(|part| {
            part.sensitivity() == SemanticSensitivity::Secret
                && matches!(
                    part.role(),
                    SemanticRole::Textbox | SemanticRole::Spinbutton
                )
        })
}

fn composer_has_send(snapshot: &SemanticSnapshot) -> bool {
    snapshot.nodes().iter().any(|node| {
        node.role() == SemanticRole::Button
            && names_any(
                &label(node),
                &["send", "post", "reply", "comment", "wyślij"],
            )
    })
}

/// The same page context governs typing and keyboard value edits.
fn typing_effect(node: &SemanticNode, snapshot: &SemanticSnapshot) -> SiteEffect {
    if in_search(node, snapshot) {
        SiteEffect::Read
    } else if node.editable_structure().is_some()
        && !ancestors(node, snapshot)
            .any(|(_, a)| a.landmark_kind() == Some(SemanticLandmarkKind::Form))
        && !composer_has_send(snapshot)
        && !composer(node)
        && draft_creator(node, snapshot).is_none()
    {
        // A document that saves as it is typed commits on the first key.
        SiteEffect::Commit(Consequence::Edit)
    } else {
        SiteEffect::Draft
    }
}

fn key_activates(key: Option<SemanticPressKey>, node: &SemanticNode) -> bool {
    match key {
        Some(SemanticPressKey::Enter) => matches!(
            node.role(),
            SemanticRole::Button | SemanticRole::Link | SemanticRole::MenuItem | SemanticRole::Tab
        ),
        Some(SemanticPressKey::Space) => matches!(
            node.role(),
            SemanticRole::Button
                | SemanticRole::Link
                | SemanticRole::Checkbox
                | SemanticRole::Radio
        ),
        _ => false,
    }
}

/// Only keys with a known value-editing meaning for this observed control.
/// Caret movement and unknown navigation controls keep their existing policy.
fn key_changes_value(key: Option<SemanticPressKey>, node: &SemanticNode) -> bool {
    let text = node.editable_structure().is_some()
        || matches!(
            node.role(),
            SemanticRole::Textbox | SemanticRole::Searchbox | SemanticRole::Combobox
        );
    match key {
        Some(SemanticPressKey::Backspace | SemanticPressKey::Delete) => text,
        Some(SemanticPressKey::Space) => {
            text || matches!(node.role(), SemanticRole::Checkbox | SemanticRole::Radio)
        }
        Some(SemanticPressKey::Enter) => node.editable_structure().is_some(),
        Some(SemanticPressKey::ArrowUp | SemanticPressKey::ArrowDown) => matches!(
            node.role(),
            SemanticRole::Spinbutton
                | SemanticRole::Slider
                | SemanticRole::Combobox
                | SemanticRole::Listbox
                | SemanticRole::Option
        ),
        Some(
            SemanticPressKey::ArrowLeft
            | SemanticPressKey::ArrowRight
            | SemanticPressKey::Home
            | SemanticPressKey::End
            | SemanticPressKey::PageUp
            | SemanticPressKey::PageDown,
        ) => node.role() == SemanticRole::Slider,
        _ => false,
    }
}

/// Reads what a step would do from the observed target and its surroundings.
pub(crate) fn classify(
    kind: SemanticActionKind,
    key: Option<SemanticPressKey>,
    node: &SemanticNode,
    snapshot: &SemanticSnapshot,
) -> SiteEffect {
    let text = label(node);
    let commits = commit_words(&text);
    match kind {
        SemanticActionKind::Scroll => SiteEffect::Read,
        SemanticActionKind::Click if super::consent::banner_choice(node, snapshot).is_some() => {
            SiteEffect::Read
        }
        SemanticActionKind::Click => {
            let dismiss = names_any(&text, &DISMISS_WORDS) && commits.is_none();
            if commits.is_none()
                && ((node.role() == SemanticRole::Tab
                    && node.activation() != Some(SemanticActivation::Submit))
                    || node.activation() == Some(SemanticActivation::Disclosure)
                    || node.role() == SemanticRole::Link
                    || node.activation() == Some(SemanticActivation::Navigation)
                    || dismiss)
            {
                SiteEffect::Read
            } else if node.role() == SemanticRole::Button && pays(node, snapshot) {
                SiteEffect::Commit(Consequence::Purchase)
            } else if node.role() == SemanticRole::Button
                && creates(&text)
                && draft_dialog(node, snapshot).is_some()
            {
                // Creating what the dialog drafted puts it in the workspace.
                SiteEffect::Commit(commits.unwrap_or(Consequence::Save))
            } else if let Some(consequence) = commits.or_else(|| dialog_commits(node, snapshot)) {
                if consequence == Consequence::Destructive
                    && edits_unsaved_input(&text, node, snapshot)
                {
                    SiteEffect::Draft
                } else {
                    SiteEffect::Commit(consequence)
                }
            } else if node.activation() == Some(SemanticActivation::Submit) {
                if in_search(node, snapshot) || queries(node) {
                    SiteEffect::Read
                } else {
                    SiteEffect::Commit(Consequence::Save)
                }
            } else {
                SiteEffect::Draft
            }
        }
        SemanticActionKind::Fill | SemanticActionKind::Select => typing_effect(node, snapshot),
        SemanticActionKind::Press if key_activates(key, node) => {
            classify(SemanticActionKind::Click, None, node, snapshot)
        }
        SemanticActionKind::Press
            if key_changes_value(key, node) && key != Some(SemanticPressKey::Enter) =>
        {
            typing_effect(node, snapshot)
        }
        SemanticActionKind::Press => match key {
            Some(SemanticPressKey::Enter)
                if node.editable_structure().is_some()
                    && typing_effect(node, snapshot) == SiteEffect::Commit(Consequence::Edit) =>
            {
                SiteEffect::Commit(Consequence::Edit)
            }
            Some(SemanticPressKey::Enter) => {
                if in_search(node, snapshot)
                    || queries(node)
                    || matches!(
                        node.role(),
                        SemanticRole::Combobox | SemanticRole::Listbox | SemanticRole::Option
                    )
                {
                    SiteEffect::Read
                } else if composer(node)
                    && (node.role() == SemanticRole::Textbox || node.editable_structure().is_some())
                    && !pays(node, snapshot)
                    && !subtree(snapshot, region_index(node, snapshot))
                        .iter()
                        .any(|part| {
                            part.role() == SemanticRole::Button
                                && matches!(
                                    commit_words(&label(part)),
                                    Some(Consequence::Purchase | Consequence::Destructive)
                                )
                        })
                {
                    // A named message composer commits a communication even
                    // when a compact look omits Send. Unrelated page buttons
                    // must not relabel Enter as saving or purchasing.
                    SiteEffect::Commit(Consequence::Communication)
                } else if let Some(creator) = draft_creator(node, snapshot) {
                    // Enter in a drafted item makes it, as its own button would.
                    SiteEffect::Commit(
                        commit_words(&label(creator))
                            .filter(|c| *c != Consequence::Destructive)
                            .unwrap_or(Consequence::Save),
                    )
                } else {
                    // Enter commits what the region's own control would.
                    let region = subtree(snapshot, region_index(node, snapshot));
                    SiteEffect::Commit(
                        region
                            .iter()
                            .filter(|part| part.role() == SemanticRole::Button)
                            .find_map(|part| commit_words(&label(part)))
                            .unwrap_or(if composer_has_send(snapshot) {
                                Consequence::Communication
                            } else {
                                Consequence::Save
                            }),
                    )
                }
            }
            Some(SemanticPressKey::Backspace | SemanticPressKey::Delete) => SiteEffect::Draft,
            Some(SemanticPressKey::Space) => match node.role() {
                SemanticRole::Button | SemanticRole::Link => {
                    classify(SemanticActionKind::Click, None, node, snapshot)
                }
                _ => SiteEffect::Draft,
            },
            _ => SiteEffect::Read,
        },
    }
}

/// A sign-in wall on this page: a visible password field, or a small page
/// asking to sign in with an account field.
/// Links past which a page with a sign-in form is the site's open page.
const OPEN_PAGE_LINKS: usize = 30;

/// A site's open landing page (GitHub's, with its sign-up email box) also
/// offers search and plenty of links; a sign-in or code wall offers neither.
fn open_page<'a>(nodes: impl IntoIterator<Item = &'a SemanticNode>) -> bool {
    let mut links = 0;
    for node in nodes {
        if node.role() == SemanticRole::Searchbox
            || node.landmark_kind() == Some(zephium_agentic::SemanticLandmarkKind::Search)
        {
            return true;
        }
        links += usize::from(node.role() == SemanticRole::Link);
    }
    links >= OPEN_PAGE_LINKS
}

pub(crate) fn sign_in_wall(observation: &SemanticObservation) -> bool {
    let Some(snapshot) = observation.frames().first() else {
        return false;
    };
    // An offer to sign in that the page lets the person close is no wall.
    let offers = super::overlay::dismissable(snapshot);
    let nodes = snapshot
        .nodes()
        .iter()
        .enumerate()
        .filter(|(index, _)| !offers.iter().any(|(range, _)| range.contains(index)))
        .map(|(_, node)| node)
        .collect::<Vec<_>>();
    // Account/tenant selectors and SSO entry pages can contain no password
    // field. They are still identity boundaries, not public-page sign-in offers.
    let chooser = nodes.iter().any(|node| {
        matches!(node.role(), SemanticRole::Heading | SemanticRole::Dialog)
            && names_any(
                &label(node),
                &[
                    "find your workspace",
                    "enter your workspace",
                    "choose a workspace",
                    "choose your workspace",
                    "select a workspace",
                    "select your workspace",
                    "choose an account",
                    "choose your account",
                    "select an account",
                    "select your account",
                    "choose an organization",
                    "select your organization",
                    "single sign on",
                    "sign in with sso",
                ],
            )
    });
    if chooser && nodes.len() <= 160 && !open_page(nodes.iter().copied()) {
        return true;
    }
    let asks = nodes.iter().any(|node| {
        matches!(node.role(), SemanticRole::Heading | SemanticRole::Button)
            && names_any(&label(node), &SIGN_IN_PHRASES)
    });
    // Password controls also appear in public examples and account settings.
    // The control alone is secret, not proof that reading the page needs login.
    // Trusted login/SSO routes are checked separately by the controller.
    if asks
        && nodes
            .iter()
            .any(|node| node.role() == SemanticRole::Password)
    {
        return true;
    }
    nodes.len() <= 160
        && asks
        && !open_page(nodes.iter().copied())
        && nodes.iter().any(|node| {
            node.role() == SemanticRole::Textbox
                && (node.sensitivity() != SemanticSensitivity::Public
                    || names_any(
                        &label(node),
                        &[
                            "email",
                            "username",
                            "phone",
                            "e-mail",
                            "workspace",
                            "organization",
                            "tenant",
                        ],
                    ))
        })
}

/// The same non-interactive wall classification applies to public research,
/// signed-in reading and results pages before their content can be extracted.
pub(crate) fn reading_wall(observation: &SemanticObservation) -> Option<AgentBrowserHumanReason> {
    if sign_in_wall(observation) {
        Some(AgentBrowserHumanReason::SignIn)
    } else if embedded_human_challenge(observation) {
        Some(AgentBrowserHumanReason::HumanChallenge)
    } else {
        secret_entry(observation)
    }
}

/// Titles a challenge widget's own frame carries. The frame is never read;
/// its title alone says a person has to pass it.
const EMBEDDED_CHALLENGE_TITLES: [&str; 5] = [
    "hcaptcha",
    "security challenge",
    "recaptcha challenge",
    "captcha challenge",
    "verify you are human",
];
/// reCAPTCHA's checkbox and the score badge many pages carry share one frame
/// title; the checkbox is the wider one (304 against 256 CSS pixels).
const RECAPTCHA_CHECKBOX_MIN_WIDTH: u32 = 290;

/// A visible challenge widget in an embedded frame, on a page of any size:
/// the agent cannot pass it and must not try.
pub(crate) fn embedded_human_challenge(observation: &SemanticObservation) -> bool {
    observation
        .frames()
        .iter()
        .flat_map(SemanticSnapshot::nodes)
        .filter(|node| node.role() == SemanticRole::FrameBoundary)
        .any(|node| {
            let (Some(name), Some(rect)) = (node.name(), node.geometry()) else {
                return false;
            };
            let name = name.as_str().trim().to_ascii_lowercase();
            EMBEDDED_CHALLENGE_TITLES
                .iter()
                .any(|title| name.contains(title))
                || (name == "recaptcha" && rect.width() >= RECAPTCHA_CHECKBOX_MIN_WIDTH)
        })
}

/// Titles payment providers give their card-field frames (Stripe, Braintree,
/// Adyen, Checkout.com and the like). The frame is never read.
const PAYMENT_FRAME_TITLES: [&str; 10] = [
    "card number",
    "card payment",
    "payment input",
    "credit card",
    "card data",
    "card details",
    "expiration date",
    "expiry date",
    "cvc",
    "cvv",
];

/// A visible card-field frame: the person enters card details, never the
/// agent, which cannot see or reach inside the frame anyway.
pub(crate) fn payment_frame(observation: &SemanticObservation) -> bool {
    observation
        .frames()
        .iter()
        .flat_map(SemanticSnapshot::nodes)
        .filter(|node| node.role() == SemanticRole::FrameBoundary && node.geometry().is_some())
        .filter_map(|node| node.name())
        .any(|name| names_any(name.as_str(), &PAYMENT_FRAME_TITLES))
}

/// Words a checkout shows around its card fields.
const PAYMENT_WORDS: [&str; 5] = ["card", "checkout", "billing", "payment", "płatność"];

/// A visible field only the person may type into: a one-time code, a card
/// number or a security code. Rust reads it as secret and the page agent
/// may never fill it, so the step goes to the person instead of a refusal
/// the model keeps meeting. Beside a checkout it is a payment, otherwise a
/// sign-in step such as a two-factor code.
pub(crate) fn secret_entry(observation: &SemanticObservation) -> Option<AgentBrowserHumanReason> {
    let snapshot = observation.frames().first()?;
    let nodes = snapshot.nodes();
    if open_page(nodes) {
        return None;
    }
    nodes
        .iter()
        .any(|node| {
            node.sensitivity() == SemanticSensitivity::Secret
                && node.role() != SemanticRole::Password
                && node.geometry().is_some()
                && !node.states().contains(SemanticState::Disabled)
                && node.operations().contains(SemanticOperationClass::Fill)
        })
        .then(|| {
            if nodes.iter().any(|node| {
                matches!(node.role(), SemanticRole::Heading | SemanticRole::Button)
                    && (names_any(&label(node), &PAYMENT_WORDS)
                        || names_any(&label(node), &PURCHASE_WORDS))
            }) {
                AgentBrowserHumanReason::SensitiveEffect
            } else {
                AgentBrowserHumanReason::SignIn
            }
        })
}

/// What the person sees before a held step runs, built only from the page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Preview {
    pub(crate) consequence: Consequence,
    pub(crate) headline: String,
    pub(crate) action: String,
    pub(crate) text: Option<String>,
    pub(crate) facts: Vec<(String, String)>,
}

impl Preview {
    fn digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"ZEPHIUM-SITE-PREVIEW-1\0");
        for part in [&self.headline, &self.action]
            .into_iter()
            .chain(self.text.as_ref())
            .chain(self.facts.iter().flat_map(|(label, value)| [label, value]))
        {
            hasher.update((part.len() as u64).to_be_bytes());
            hasher.update(part.as_bytes());
        }
        hasher.update([self.consequence as u8]);
        hasher.finalize().into()
    }
}

const MAX_FACTS: usize = 12;
const MAX_LEDGER: usize = 8;
const MAX_LINE: usize = 300;
const MAX_TEXT: usize = 4096;

fn clip(text: &str, limit: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.len() <= limit {
        return text;
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

fn value_text(node: &SemanticNode) -> Option<&str> {
    match node.value() {
        Some(SemanticValueSummary::Text(text))
            if node.sensitivity() == SemanticSensitivity::Public =>
        {
            Some(text.preview().text()).filter(|text| !text.trim().is_empty())
        }
        _ => None,
    }
}

/// A displayed amount such as "$1,240.00", "1 240 zł" or "€89".
fn money(text: &str) -> Option<(f64, String)> {
    const CURRENCY: [&str; 9] = ["$", "€", "£", "zł", "pln", "usd", "eur", "gbp", "¥"];
    let lower = text.to_lowercase();
    if !CURRENCY.iter().any(|mark| lower.contains(mark)) {
        return None;
    }
    let start = text.find(|ch: char| ch.is_ascii_digit())?;
    let digits: String = text[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || matches!(ch, ',' | '.' | ' ' | '\u{a0}'))
        .collect();
    let amount = digits
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect::<String>()
        .parse::<f64>()
        .ok()?;
    let from = text[..start]
        .rfind(|ch: char| ch.is_whitespace())
        .map_or(0, |at| at + 1);
    let end = start + digits.trim_end().len();
    let after = text[end..]
        .split_whitespace()
        .next()
        .filter(|word| {
            CURRENCY
                .iter()
                .any(|mark| word.to_lowercase().starts_with(mark))
        })
        .map_or(String::new(), |word| format!(" {word}"));
    Some((amount, format!("{}{after}", text[from..end].trim())))
}

fn verb(text: &str, fallback: &str) -> String {
    let name = clip(text, 60);
    if name.is_empty() || name.len() > 40 {
        fallback.to_owned()
    } else {
        name
    }
}

/// Builds the person's card from the observed region around the target.
pub(crate) fn preview(
    consequence: Consequence,
    action: &SemanticPreparedAction,
    node: &SemanticNode,
    snapshot: &SemanticSnapshot,
    site: &str,
) -> Preview {
    let region = subtree(snapshot, region_index(node, snapshot));
    let name = clip(&label(node), 80);
    let heading = region
        .iter()
        .chain(snapshot.nodes())
        .find(|part| part.role() == SemanticRole::Heading && !label(part).trim().is_empty())
        .map(|part| clip(&label(part), 80));
    let headline = match consequence {
        Consequence::Communication => {
            let verb = verb(&name, "Send");
            match &heading {
                Some(place) => {
                    format!("{verb} to {} as you?", place.trim_start_matches("Message "))
                }
                None => format!("{verb} on {site} as you?"),
            }
        }
        Consequence::Purchase => {
            let total = region
                .iter()
                .filter_map(|part| {
                    let text = label(part);
                    money(&text).map(|(amount, shown)| {
                        (names_any(&text, &["total", "razem", "suma"]), amount, shown)
                    })
                })
                .max_by(|a, b| {
                    (a.0, a.1)
                        .partial_cmp(&(b.0, b.1))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            let verb = verb(&name, "Book");
            match total {
                Some((_, _, shown)) => format!("{verb} for {shown}?"),
                None => format!("{verb} on {site}?"),
            }
        }
        Consequence::Destructive => {
            let verb = verb(&name, "Delete");
            match &heading {
                Some(item) => format!("{verb} {item}?"),
                None => format!("{verb} on {site}?"),
            }
        }
        Consequence::Save | Consequence::Edit => format!("Save changes on {site}?"),
        Consequence::Type => format!("Type on {site}?"),
    };
    let action_line = match action.kind() {
        SemanticActionKind::Fill => format!(
            "type into {}",
            if name.is_empty() { "the page" } else { &name }
        ),
        SemanticActionKind::Press => match action.bound_action().press_key() {
            Some(key) => format!("press {key:?} in {name}"),
            None => format!("press a key in {name}"),
        },
        _ => format!(
            "press {}",
            if name.is_empty() {
                "the control"
            } else {
                &name
            }
        ),
    };
    let text = match consequence {
        Consequence::Edit | Consequence::Type => {
            action.fill_text().map(|text| text.as_str().to_owned())
        }
        Consequence::Communication => (action.kind() == SemanticActionKind::Press)
            .then(|| value_text(node))
            .flatten()
            .or_else(|| {
                region
                    .iter()
                    .filter(|part| {
                        part.role() == SemanticRole::Textbox || part.editable_structure().is_some()
                    })
                    .filter(|part| !in_search(part, snapshot))
                    .find_map(value_text)
            })
            .map(str::to_owned),
        _ => None,
    }
    .map(|text| clip(&text, MAX_TEXT));
    let mut facts = Vec::new();
    for part in region {
        if facts.len() == MAX_FACTS {
            break;
        }
        if part.sensitivity() != SemanticSensitivity::Public || std::ptr::eq(part, node) {
            continue;
        }
        let fact = match part.role() {
            SemanticRole::Textbox | SemanticRole::Combobox | SemanticRole::Spinbutton
                if consequence != Consequence::Communication =>
            {
                part.name()
                    .zip(value_text(part))
                    .map(|(name, value)| (name.as_str().to_owned(), value.to_owned()))
            }
            SemanticRole::Paragraph
            | SemanticRole::Cell
            | SemanticRole::ListItem
            | SemanticRole::Status => label(part)
                .split_once(':')
                .map(|(label, value)| (label.trim().to_owned(), value.trim().to_owned()))
                .filter(|(label, value)| {
                    !label.is_empty() && !value.is_empty() && label.len() <= 60
                }),
            _ => None,
        };
        if let Some((label, value)) = fact {
            facts.push((clip(&label, MAX_LINE), clip(&value, MAX_LINE)));
        }
    }
    Preview {
        consequence,
        headline: clip(&headline, MAX_LINE),
        action: clip(&action_line, MAX_LINE),
        text,
        facts,
    }
}

/// Content, required proof and opaque native element identity promised by an approval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Fingerprint {
    approval: Option<zephium_agentic::SemanticApprovalBinding>,
    context: zephium_agentic::ContextId,
    profile: zephium_core::ids::ProfileId,
    frame: zephium_agentic::FrameId,
    option: Option<(Option<SemanticValueSummary>, [bool; 6], [u8; 32])>,
    structure: [u8; 32],
    verification: SemanticVerification,
    value: Option<SemanticValueSummary>,
    nonfocus_states: [bool; 6],
}

/// Binds the full retained value without exposing it through the public,
/// potentially truncated display preview. Focus changes are expected at handoff.
pub(crate) fn fingerprint(
    action: &SemanticPreparedAction,
    node: &SemanticNode,
    snapshot: &SemanticSnapshot,
) -> Fingerprint {
    Fingerprint {
        approval: action.approval_binding(),
        context: snapshot.frame().context().identity().id(),
        profile: snapshot.frame().context().identity().profile(),
        frame: snapshot.frame().frame(),
        option: action.option_node(snapshot).map(|option| {
            (
                option.value().cloned(),
                nonfocus_states(option),
                Sha256::digest(label(option).as_bytes()).into(),
            )
        }),
        structure: structure(action, node, snapshot),
        verification: action.verification(),
        value: node.value().cloned(),
        nonfocus_states: nonfocus_states(node),
    }
}

fn nonfocus_states(node: &SemanticNode) -> [bool; 6] {
    [
        SemanticState::Checked,
        SemanticState::Selected,
        SemanticState::Expanded,
        SemanticState::Disabled,
        SemanticState::Required,
        SemanticState::Invalid,
    ]
    .map(|state| node.states().contains(state))
}

impl Fingerprint {
    /// The same step, whichever native element currently carries it.
    fn same_step(&self, other: &Self) -> bool {
        let unbound = |fingerprint: &Self| Self {
            approval: None,
            ..fingerprint.clone()
        };
        unbound(self) == unbound(other)
    }
    fn same_page_instance(&self, other: &Self) -> bool {
        matches!((self.approval, other.approval), (Some(a), Some(b)) if a.same_keyspace(&b))
    }
}

/// Whether exactly one control on the page reads as this step's target.
fn sole_match(
    action: &SemanticPreparedAction,
    snapshot: &SemanticSnapshot,
    structure_hash: [u8; 32],
) -> bool {
    snapshot
        .nodes()
        .iter()
        .filter(|node| structure(action, node, snapshot) == structure_hash)
        .count()
        == 1
}

/// What the step is, apart from which native element holds it: its kind and
/// text, the control's role, label and ancestors.
fn structure(
    action: &SemanticPreparedAction,
    node: &SemanticNode,
    snapshot: &SemanticSnapshot,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ZEPHIUM-SITE-STEP-1\0");
    let mut part = |bytes: &[u8]| {
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    };
    part(snapshot.frame().origin().as_url().as_str().as_bytes());
    part(format!("{:?}", action.kind()).as_bytes());
    part(format!("{:?}", action.bound_action().press_key()).as_bytes());
    part(
        action
            .fill_text()
            .map_or("", |text| text.as_str())
            .as_bytes(),
    );
    part(
        format!(
            "{:?}|{:?}|{:?}",
            node.role(),
            node.activation(),
            node.form()
        )
        .as_bytes(),
    );
    part(label(node).as_bytes());
    for (_, ancestor) in ancestors(node, snapshot) {
        part(format!("{:?}|{:?}", ancestor.role(), ancestor.landmark_kind()).as_bytes());
        part(ancestor.name().map_or("", |name| name.as_str()).as_bytes());
    }
    hasher.finalize().into()
}

/// A held step waiting for the person.
#[derive(Clone, Debug)]
pub(crate) struct Pending {
    pub(crate) fingerprint: Fingerprint,
    pub(crate) digest: [u8; 32],
    pub(crate) preview: Preview,
}

/// How an approved step ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Receipt {
    /// It ran and the page showed its effect.
    Committed,
    /// It ran; the page did not show its effect before the page ended.
    Unverified,
    /// It never ran: the page changed first or the step was not taken.
    NotSent,
    Declined,
}

#[derive(Default)]
struct GateState {
    site: String,
    allow_edits: bool,
    /// Typing on this site waits for the person; see `Consequence::Type`.
    hold_typing: bool,
    /// The person allowed typing on this site for the run.
    allow_typing: bool,
    pending: Option<Pending>,
    /// One approved step, consumed by the first matching proposal.
    confirmed: Option<Pending>,
    /// The approved step was handed to the page and awaits verification.
    dispatched: Option<Consequence>,
    receipt: Option<Receipt>,
    verified_approval: bool,
    declined: Vec<[u8; 32]>,
    /// Approved steps that never ran because the page changed under them, by
    /// step and preview: the same step proposed again runs without a new card.
    unspent: Vec<([u8; 32], [u8; 32])>,
    /// The commit handed to the page and not yet verified.
    in_flight: Option<Flight>,
    /// What this page task committed, in order, with the control's words.
    committed: Vec<(Consequence, String)>,
    /// Commits that reached the page with their outcome unseen: never run
    /// again in this page task.
    unknown: Vec<Flight>,
    /// Add-to-cart controls already pressed: a second press adds a second item.
    carted: Vec<[u8; 32]>,
    /// Status lines seen before the last step, for the commit detector.
    status_before: Vec<String>,
    unconfirmed: Option<String>,
    /// A committing step was held with no one to ask.
    held: bool,
    /// The question put to the person, and whether they answered it.
    ask: Option<(u32, bool)>,
    /// Someone was asked at least once.
    asked: bool,
    /// The entry question waits on the start page's first view.
    entry: EntryCheck,
    /// Rust pressed a cookie banner's refusal on this page.
    consent_pressed: bool,
    /// The task reads a daily app's view as its records.
    view: bool,
    /// The original task expects the person's account, even outside app views.
    account_expected: bool,
    /// The app's views still to open for the read, in order.
    views: Vec<zephium_agentic::AppView>,
    /// The views Rust opened, oldest first.
    opened: Vec<zephium_agentic::AppView>,
}

/// A commit handed to the page, by what it is.
struct Flight {
    consequence: Consequence,
    control: String,
    structure: [u8; 32],
    /// The person's approval it spends, by step and preview.
    approved: Option<([u8; 32], [u8; 32])>,
    /// The cart item it adds.
    cart: Option<[u8; 32]>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum EntryCheck {
    #[default]
    Settled,
    /// The first view decides: signed out goes on, anything else asks.
    Pending,
    /// The person is being asked before the model sees the page.
    Asking,
}

/// Shared by a page task and its successors: the held step, the one approved
/// step, declines and what the page did after each step.
#[derive(Default)]
pub(crate) struct SiteGate(Mutex<GateState>);

impl SiteGate {
    pub(crate) fn new(site: String, allow_edits: bool, entry: bool) -> Self {
        Self(Mutex::new(GateState {
            site,
            allow_edits,
            entry: if entry {
                EntryCheck::Pending
            } else {
                EntryCheck::Settled
            },
            ..GateState::default()
        }))
    }
    /// The task only reads what a daily app's views list, going through
    /// the views its goal asks for.
    pub(crate) fn expecting_account(self, expected: bool) -> Self {
        self.state().account_expected = expected;
        self
    }
    pub(crate) fn reading_view(self, url: &str, goal: &str) -> Self {
        {
            let mut state = self.state();
            state.view = true;
            state.views = zephium_agentic::ContextNavigationTarget::parse(url)
                .ok()
                .and_then(|url| {
                    url.as_url()
                        .host_str()
                        .and_then(zephium_agentic::DailyApp::of)
                })
                .map(|app| zephium_agentic::app_views(app, goal).to_vec())
                .unwrap_or_default();
        }
        self
    }
    /// Every field typed into on this site waits for the person, until they
    /// allow typing on it for the run.
    pub(crate) fn holding_typing(self, hold: bool) -> Self {
        self.state().hold_typing = hold;
        self
    }
    /// Which views the read went through, for the helper: nothing new in the
    /// first ones, the rows from the last.
    pub(crate) fn view_note(&self) -> Option<String> {
        let state = self.state();
        let (last, before) = state.opened.split_last()?;
        let names = |views: &[zephium_agentic::AppView]| {
            views
                .iter()
                .map(|view| view.name())
                .collect::<Vec<_>>()
                .join(" and ")
        };
        Some(match (before.is_empty(), last.latest()) {
            (true, _) => format!("Read from the {} view.", last.name()),
            (false, true) => format!(
                "Nothing new in {}: these are the latest in {}.",
                names(before),
                last.name()
            ),
            (false, false) => format!(
                "Nothing new in {}; read from {}.",
                names(before),
                last.name()
            ),
        })
    }
    pub(crate) fn consent_pressing(&self, pressing: bool) {
        self.state().consent_pressed |= pressing;
    }
    pub(crate) fn pressed_consent(&self) -> bool {
        self.state().consent_pressed
    }
    pub(crate) fn entry(&self) -> EntryCheck {
        self.state().entry
    }
    /// The person answered the entry question; the page goes on.
    pub(crate) fn entered(&self) {
        self.state().entry = EntryCheck::Settled;
    }
    fn state(&self) -> std::sync::MutexGuard<'_, GateState> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub(crate) fn pending(&self) -> Option<Pending> {
        self.state().pending.clone()
    }
    pub(crate) fn allow_edits(&self) -> bool {
        self.state().allow_edits
    }
    /// The one extra effect a successor may carry: the approved step's.
    pub(crate) fn approved_class(&self) -> Option<SemanticEffectClass> {
        self.state()
            .confirmed
            .as_ref()
            .map(|pending| pending.preview.consequence.class())
    }
    /// The person approved the held step; `for_run` also allows later edits,
    /// or later typing when typing was what it held.
    pub(crate) fn approve(&self, for_run: bool) -> Option<Preview> {
        let mut state = self.state();
        let pending = state.pending.take()?;
        if for_run && pending.preview.consequence == Consequence::Type {
            state.allow_typing = true;
        } else if for_run {
            state.allow_edits = true;
        }
        let preview = pending.preview.clone();
        state.confirmed = Some(pending);
        state.receipt = None;
        Some(preview)
    }
    pub(crate) fn decline(&self) -> Option<Preview> {
        let mut state = self.state();
        let pending = state.pending.take()?;
        state.declined.push(pending.fingerprint.structure);
        state.receipt = Some(Receipt::Declined);
        Some(pending.preview)
    }
    /// Takes how the last approved step ended, once known.
    pub(crate) fn take_receipt(&self) -> Option<Receipt> {
        self.state().receipt.take()
    }
    pub(crate) fn has_verified_approval(&self) -> bool {
        self.state().verified_approval
    }
    /// The page ended: an approved step still waiting ends here too.
    pub(crate) fn finish(&self) -> Option<Receipt> {
        let mut state = self.state();
        if state.dispatched.take().is_some() {
            return Some(Receipt::Unverified);
        }
        if state.confirmed.take().is_some() {
            return Some(Receipt::NotSent);
        }
        state
            .receipt
            .take()
            .or_else(|| (!state.unspent.is_empty()).then_some(Receipt::NotSent))
    }
    /// The step handed to the page never reached it: an approval it spent
    /// goes back to that step, and an item it added is not counted.
    fn not_run(&self) {
        let mut state = self.state();
        let Some(flight) = state.in_flight.take() else {
            return;
        };
        if state.dispatched.take().is_some() {
            state.unspent.extend(flight.approved);
        }
        if let Some(item) = flight.cart {
            state.carted.retain(|known| *known != item);
        }
    }
    /// The step reached the page and its outcome was not seen. Typing
    /// commits nothing, so it may run again; any other step may have
    /// committed and never runs again.
    fn outcome_unknown(&self) {
        let mut state = self.state();
        let Some(flight) = state.in_flight.take() else {
            return;
        };
        if flight.consequence == Consequence::Type {
            if state.dispatched.take().is_some() {
                state.unspent.extend(flight.approved);
            }
            return;
        }
        if state.dispatched.take().is_some() {
            state.receipt = Some(Receipt::Unverified);
        }
        state.unknown.push(flight);
    }
    /// What this page task committed, one plain line each: "Saved on the page:
    /// Add task". The lead reports an item as made by these, whatever the
    /// page agent's findings say.
    pub(crate) fn committed(&self) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for (consequence, control) in &self.state().committed {
            let verb = match consequence {
                Consequence::Communication => "Sent",
                Consequence::Destructive => "Deleted",
                Consequence::Purchase => "Purchased",
                Consequence::Save | Consequence::Edit => "Saved",
                // Typing that waited for the person commits nothing itself.
                Consequence::Type => continue,
            };
            let line = if control.is_empty() {
                format!("{verb} on the page")
            } else {
                format!("{verb} on the page: {control}")
            };
            if lines.last() != Some(&line) && lines.len() < MAX_LEDGER {
                lines.push(line);
            }
        }
        for flight in &self.state().unknown {
            let verb = match flight.consequence {
                Consequence::Communication => "sent",
                Consequence::Destructive => "deleted",
                Consequence::Purchase => "purchased",
                Consequence::Save | Consequence::Edit => "saved",
                Consequence::Type => continue,
            };
            if lines.len() < MAX_LEDGER {
                lines.push(format!(
                    "Maybe {verb} on the page, not confirmed: {}",
                    flight.control
                ));
            }
        }
        lines
    }
    pub(crate) fn unconfirmed(&self) -> Option<String> {
        self.state().unconfirmed.clone()
    }
    pub(crate) fn ask(&self) -> Option<(u32, bool)> {
        self.state().ask
    }
    pub(crate) fn set_ask(&self, ask: Option<(u32, bool)>) {
        let mut state = self.state();
        state.asked |= ask.is_some();
        state.ask = ask;
    }
    /// Held back with the question unanswered, or with no one to ask.
    pub(crate) fn held_back(&self) -> bool {
        let state = self.state();
        state.pending.is_some() || (state.held && !state.asked)
    }
}

/// A short line that reports something just done: its done word opens or
/// closes it ("Message sent", "Booking confirmed", "Deleted 3 items"), or it
/// says so outright ("Your order has been placed", "Thank you for your
/// order"). A listing heading that only uses the word ("Most booked hotels
/// in Krakow") is not a report.
fn says_done(line: &str) -> bool {
    let line = words(line);
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() || parts.len() > 8 || !names_any(&line, &DONE_WORDS) {
        return false;
    }
    // An editor keeping what is being written ("Draft saved") commits nothing.
    if names_any(&line, &["draft", "drafts", "autosaved", "szkic", "wersja robocza"]) {
        return false;
    }
    let edge = |word: &str| {
        DONE_WORDS
            .iter()
            .any(|done| !done.contains(' ') && *done == word)
    };
    edge(parts[0])
        || edge(parts[parts.len() - 1])
        || DONE_WORDS
            .iter()
            .any(|done| done.contains(' ') && line.contains(&format!(" {done} ")))
        || names_any(&line, &["has been", "have been", "was", "were", "is now"])
}

fn status_lines(observation: &SemanticObservation) -> Vec<String> {
    observation
        .frames()
        .iter()
        .flat_map(|frame| frame.nodes())
        .filter(|node| matches!(node.role(), SemanticRole::Status | SemanticRole::Heading))
        .map(|node| clip(&label(node), MAX_LINE))
        .filter(|line| !line.is_empty())
        .collect()
}

/// One item's add-to-cart control: the control and the page's own title
/// and main heading, so another product's button is another item.
fn cart_item(fingerprint: &Fingerprint, snapshot: &SemanticSnapshot) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(fingerprint.structure);
    let nodes = snapshot.nodes();
    let title = nodes.iter().find(|node| {
        node.name()
            .is_some_and(|name| name.as_str() == "Page title")
    });
    let heading = nodes
        .iter()
        .find(|node| node.role() == SemanticRole::Heading);
    for node in [title, heading].into_iter().flatten() {
        hasher.update(label(node).as_bytes());
        hasher.update([0]);
    }
    hasher.finalize().into()
}

pub(crate) struct SiteWorkPolicy {
    pub(crate) gate: Arc<SiteGate>,
    /// Someone can be asked; without it a committing step is only held back.
    pub(crate) asks: bool,
}

/// Controls a signed-in page shows for its account.
const ACCOUNT_WORDS: [&str; 10] = [
    "account",
    "profile",
    "avatar",
    "my account",
    "sign out",
    "log out",
    "logout",
    "wyloguj",
    "konto",
    "your profile",
];

/// The start page reads as signed out: a sign-in wall, or a sign-in control
/// in its header or navigation with no account control anywhere.
pub(crate) fn signed_out(observation: &SemanticObservation) -> bool {
    if sign_in_wall(observation) {
        return true;
    }
    let Some(snapshot) = observation.frames().first() else {
        return false;
    };
    let controls = || {
        snapshot
            .nodes()
            .iter()
            .filter(|node| matches!(node.role(), SemanticRole::Link | SemanticRole::Button))
    };
    let offers = controls().any(|node| {
        names_any(&label(node), &SIGN_IN_PHRASES)
            && ancestors(node, snapshot).any(|(_, ancestor)| {
                matches!(
                    ancestor.landmark_kind(),
                    Some(SemanticLandmarkKind::Banner | SemanticLandmarkKind::Navigation)
                )
            })
    });
    offers && !controls().any(|node| names_any(&label(node), &ACCOUNT_WORDS))
}

/// A field typing into which commits nothing by itself: a search box, a
/// place or date picker, a filter, a field of a form that only queries. A
/// field of a form that posts, a personal or secret one, or one that saves
/// as it is typed stays verified exactly.
fn rewritable(kind: SemanticActionKind, node: &SemanticNode, snapshot: &SemanticSnapshot) -> bool {
    matches!(kind, SemanticActionKind::Fill | SemanticActionKind::Select)
        && node.sensitivity() == SemanticSensitivity::Public
        && !credential(node)
        && (in_search(node, snapshot)
            || node
                .form()
                .is_none_or(|form| form.method() == SemanticFormMethod::Get || form.search()))
        && !matches!(classify(kind, None, node, snapshot), SiteEffect::Commit(_))
}

fn credential(node: &SemanticNode) -> bool {
    node.role() == SemanticRole::Password || node.sensitivity() == SemanticSensitivity::Secret
}

impl AgentWorkLocalActionPolicy for SiteWorkPolicy {
    fn decision_action_recipe(
        &self,
        operation: &zephium_agentic::DecisionOperation,
        observation: &SemanticObservation,
    ) -> Result<
        Option<zephium_agentic::SemanticActionProposal>,
        zephium_agent_controller::AgentWorkFailure,
    > {
        // The pilot executes only existing noncommitting read recipes. Text,
        // unsupported controls and consequential actions stay with the planner.
        let recipe = super::read_interactions::ReadingInteractionPolicy
            .decision_action_recipe(operation, observation)?;
        Ok(recipe.filter(|recipe| {
            observation.frames().iter().any(|frame| {
                frame.nodes().iter().any(|node| {
                    node.reference() == recipe.intent().target()
                        && classify(recipe.intent().kind(), None, node, frame) == SiteEffect::Read
                })
            })
        }))
    }

    fn allows_compound_forms(&self) -> bool {
        true
    }

    fn consent_dismissal(&self, observation: &SemanticObservation) -> Option<SemanticReferenceId> {
        super::consent::dismissal(observation)
    }

    fn consent_suspected(&self, observation: &SemanticObservation) -> bool {
        super::consent::suspected(observation)
    }

    fn overlay_dismissal(&self, observation: &SemanticObservation) -> Option<SemanticReferenceId> {
        super::overlay::dismissal(observation)
    }

    fn consent_pressing(&self, pressing: bool) {
        self.gate.consent_pressing(pressing);
    }

    fn whole_first_look(&self) -> bool {
        self.gate.entry() == EntryCheck::Pending
    }

    fn reads_app_view(&self) -> bool {
        self.gate.state().view
    }

    fn app_view(&self, observation: &SemanticObservation) -> Option<SemanticReferenceId> {
        let mut state = self.gate.state();
        while !state.views.is_empty() {
            let view = state.views.remove(0);
            if let Some(control) = zephium_agentic::app_view_control(observation, view) {
                state.opened.push(view);
                return Some(control);
            }
        }
        None
    }

    fn sign_in_optional(&self, observation: &SemanticObservation) -> bool {
        let state = self.gate.state();
        !state.view
            && !state.account_expected
            && !sign_in_wall(observation)
            && secret_entry(observation).is_none()
    }

    fn patient_checks(&self) -> bool {
        true
    }

    fn human_wall(&self, observation: &SemanticObservation) -> Option<AgentBrowserHumanReason> {
        let mut state = self.gate.state();
        if state.entry == EntryCheck::Pending {
            state.entry = if signed_out(observation) {
                EntryCheck::Settled
            } else {
                EntryCheck::Asking
            };
        }
        if state.entry == EntryCheck::Asking {
            return Some(AgentBrowserHumanReason::UserDecision);
        }
        if state.unconfirmed.is_some() {
            return Some(AgentBrowserHumanReason::Verification);
        }
        if state.pending.is_some() && self.asks {
            return Some(AgentBrowserHumanReason::UserDecision);
        }
        let account = state.view || state.account_expected;
        drop(state);
        // A signed-out app can show its public landing page instead of a
        // wall; a task about the person's own account has nothing to read
        // there, however much text the page holds.
        if sign_in_wall(observation) || (account && signed_out(observation)) {
            return Some(AgentBrowserHumanReason::SignIn);
        }
        if embedded_human_challenge(observation) {
            return Some(AgentBrowserHumanReason::HumanChallenge);
        }
        if payment_frame(observation) {
            return Some(AgentBrowserHumanReason::SensitiveEffect);
        }
        secret_entry(observation)
    }

    fn model_action_operations(
        &self,
        node: &SemanticNode,
        _: &SemanticObservation,
    ) -> Result<SemanticOperations, AgentWorkFailure> {
        if node.states().contains(SemanticState::Disabled) || node.geometry().is_none() {
            return Ok(SemanticOperations::NONE);
        }
        let operations = [
            SemanticOperationClass::Click,
            SemanticOperationClass::Fill,
            SemanticOperationClass::Select,
            SemanticOperationClass::Press,
            SemanticOperationClass::Scroll,
        ]
        .into_iter()
        .filter(|operation| node.operations().contains(*operation))
        .filter(|operation| {
            !credential(node)
                || matches!(
                    operation,
                    SemanticOperationClass::Click | SemanticOperationClass::Scroll
                )
        })
        .collect::<Vec<_>>();
        SemanticOperations::try_new(&operations).map_err(|_| AgentWorkFailure::Contract)
    }

    fn rewritable_field(
        &self,
        action: &SemanticPreparedAction,
        observation: &SemanticObservation,
    ) -> bool {
        let Some(snapshot) = observation.frames().first() else {
            return false;
        };
        let Some(node) = action.target_node(snapshot) else {
            return false;
        };
        matches!(
            action.effect(),
            SemanticEffectClass::Read | SemanticEffectClass::LocalWrite
        ) && !self.gate.state().hold_typing
            && rewritable(action.kind(), node, snapshot)
    }

    fn assess(
        &self,
        action: &SemanticPreparedAction,
        observation: &SemanticObservation,
    ) -> Result<AgentEffectAssessment, AgentWorkFailure> {
        let snapshot = observation
            .frames()
            .first()
            .ok_or(AgentWorkFailure::Contract)?;
        let node = action
            .target_node(snapshot)
            .ok_or(AgentWorkFailure::Contract)?;
        self.gate.state().in_flight = None;
        if credential(node)
            && matches!(
                action.kind(),
                SemanticActionKind::Fill | SemanticActionKind::Press | SemanticActionKind::Select
            )
        {
            return Err(AgentWorkFailure::ActionDenied);
        }
        let declared = action.effect();
        // A banner choice is a read: refused optional cookies, a notice
        // closed or its settings opened. Accepting is refused while the same
        // banner offers a refusal.
        if action.kind() == SemanticActionKind::Click
            || (action.kind() == SemanticActionKind::Press
                && key_activates(action.bound_action().press_key(), node))
        {
            if let Some(choice) = super::consent::banner_choice(node, snapshot) {
                if choice == super::consent::Choice::Accept
                    && super::consent::offers_refusal(node, snapshot)
                {
                    return Err(AgentWorkFailure::ActionDenied);
                }
                if declared != SemanticEffectClass::Read {
                    return Err(AgentWorkFailure::EffectRequired(SemanticEffectClass::Read));
                }
                return Ok(AgentEffectAssessment::new(
                    action,
                    action.frame().origin().clone(),
                    declared,
                ));
            }
        }
        let accept = || {
            Ok(AgentEffectAssessment::new(
                action,
                action.frame().origin().clone(),
                declared,
            ))
        };
        let observed = classify(
            action.kind(),
            action.bound_action().press_key(),
            node,
            snapshot,
        );
        let keyboard_mutation = action.kind() == SemanticActionKind::Press
            && key_changes_value(action.bound_action().press_key(), node);
        if keyboard_mutation {
            let mut state = self.gate.state();
            // A key's affected selection/caret is not captured in the preview.
            // Never spend an exact one-step approval on that unknown edit.
            if (observed == SiteEffect::Commit(Consequence::Edit) && !state.allow_edits)
                || (state.hold_typing && !state.allow_typing)
            {
                state.held = true;
                return Err(AgentWorkFailure::ActionDenied);
            }
            if observed == SiteEffect::Draft && declared == SemanticEffectClass::Read {
                return Err(AgentWorkFailure::EffectRequired(
                    SemanticEffectClass::LocalWrite,
                ));
            }
        }
        let held_typing = matches!(
            action.kind(),
            SemanticActionKind::Fill | SemanticActionKind::Select
        ) && {
            let state = self.gate.state();
            state.hold_typing && !state.allow_typing
        };
        // The more consequential side wins; neither lowers the other.
        let consequence = match (observed, Consequence::declared(declared)) {
            (SiteEffect::Commit(consequence), _) => consequence,
            (_, Some(consequence)) => consequence,
            _ if held_typing => Consequence::Type,
            (_, None) if declared == SemanticEffectClass::CapabilityBoundary => {
                return Err(AgentWorkFailure::ActionDenied);
            }
            (_, None) => {
                self.gate.state().status_before = status_lines(observation);
                return accept();
            }
        };
        let mut state = self.gate.state();
        let Some(current_binding) = action.approval_binding() else {
            state.held = true;
            return Err(AgentWorkFailure::ActionDenied);
        };
        let accept_approved = |binding| {
            AgentEffectAssessment::with_approval(
                action,
                action.frame().origin().clone(),
                declared,
                binding,
            )
            .ok_or(AgentWorkFailure::ActionDenied)
        };
        let fingerprint = fingerprint(action, node, snapshot);
        let site = state.site.clone();
        let preview = preview(consequence, action, node, snapshot, &site);
        let digest = preview.digest();
        let fits = if consequence == Consequence::Type {
            matches!(
                declared,
                SemanticEffectClass::Read | SemanticEffectClass::LocalWrite
            )
        } else {
            declared == consequence.class()
        };
        let control = clip(&label(node), 60);
        let flight = |approved| Flight {
            consequence,
            control: control.clone(),
            structure: fingerprint.structure,
            approved,
            cart: None,
        };
        // It reached the page before and its outcome was never seen: it may
        // have committed, so it is not run again.
        if consequence != Consequence::Type
            && state
                .unknown
                .iter()
                .any(|flight| flight.structure == fingerprint.structure)
        {
            return Err(AgentWorkFailure::OutcomeUnknown);
        }
        // The person approved this step and the page then changed under it
        // without changing what it is: it runs, once, without a new question.
        if let Some(at) = state
            .unspent
            .iter()
            .position(|(step, preview)| *step == fingerprint.structure && *preview == digest)
        {
            if !fits {
                return Err(AgentWorkFailure::EffectRequired(consequence.class()));
            }
            let approved = state.unspent.remove(at);
            state.dispatched = Some(consequence);
            state.in_flight = Some(flight(Some(approved)));
            state.status_before = status_lines(observation);
            return accept_approved(current_binding);
        }
        if let Some(confirmed) = state.confirmed.take() {
            let same_step =
                confirmed.digest == digest && confirmed.fingerprint.same_step(&fingerprint);
            // An SPA re-renders the node while the card waits: the approval
            // follows the step to its new element when exactly one control
            // on the page is that step.
            if same_step
                && (confirmed.fingerprint.approval == fingerprint.approval
                    || (confirmed.fingerprint.same_page_instance(&fingerprint)
                        && sole_match(action, snapshot, fingerprint.structure)))
            {
                if !fits {
                    state.confirmed = Some(confirmed);
                    return Err(AgentWorkFailure::EffectRequired(consequence.class()));
                }
                state.dispatched = Some(consequence);
                state.in_flight = Some(flight(Some((
                    confirmed.fingerprint.structure,
                    confirmed.digest,
                ))));
                state.status_before = status_lines(observation);
                return accept_approved(current_binding);
            }
            // Edits allowed for the run: the approval covers this edit too.
            if matches!(consequence, Consequence::Edit | Consequence::Save)
                && state.allow_edits
                && confirmed.preview.consequence == consequence
            {
                if declared != SemanticEffectClass::ExternalWrite {
                    state.confirmed = Some(confirmed);
                    return Err(AgentWorkFailure::EffectRequired(
                        SemanticEffectClass::ExternalWrite,
                    ));
                }
                state.dispatched = Some(consequence);
                state.in_flight = Some(flight(None));
                state.status_before = status_lines(observation);
                return accept_approved(current_binding);
            }
            // The page changed since the person decided: that approval is spent.
            state.receipt = Some(Receipt::NotSent);
            if same_step {
                // The step is what they approved, but more than one control
                // reads as it: the same proposal again runs, with no new card.
                state
                    .unspent
                    .push((confirmed.fingerprint.structure, confirmed.digest));
                return Err(AgentWorkFailure::ActionDenied);
            }
        }
        // Saving or editing what the person asked for in their own app is
        // the request itself; sending, buying and deleting still wait.
        if matches!(consequence, Consequence::Edit | Consequence::Save) && state.allow_edits {
            if declared != SemanticEffectClass::ExternalWrite {
                return Err(AgentWorkFailure::EffectRequired(
                    SemanticEffectClass::ExternalWrite,
                ));
            }
            // The planner pressed Add to cart again after the page said it
            // was added, putting the item in three times.
            let mut flight = flight(None);
            if names_any(&label(node), &CART_WORDS) {
                let item = cart_item(&fingerprint, snapshot);
                if state.carted.contains(&item) {
                    return Err(AgentWorkFailure::AlreadyDone);
                }
                state.carted.push(item);
                flight.cart = Some(item);
            }
            state.in_flight = Some(flight);
            state.status_before = status_lines(observation);
            return accept_approved(current_binding);
        }
        if state.declined.contains(&fingerprint.structure) {
            return Err(AgentWorkFailure::ActionDenied);
        }
        state.held = true;
        if self.asks {
            state.pending = Some(Pending {
                fingerprint,
                digest,
                preview,
            });
        }
        Err(AgentWorkFailure::ActionDenied)
    }

    fn accept_verified_action(
        &mut self,
        _: &SemanticActionBatchResult,
        observation: &SemanticObservation,
    ) -> Result<(), AgentWorkFailure> {
        self.after_step(observation);
        Ok(())
    }

    fn action_not_run(&self) {
        self.gate.not_run();
    }

    fn action_outcome_unknown(&self) {
        self.gate.outcome_unknown();
    }
}

impl SiteWorkPolicy {
    /// Records how a verified step ended: the approved one committed, or
    /// the page said a step Rust read as harmless committed something.
    fn after_step(&self, observation: &SemanticObservation) {
        let mut state = self.gate.state();
        if let Some(flight) = state.in_flight.take() {
            state.committed.push((flight.consequence, flight.control));
        }
        if state.dispatched.take().is_some() {
            state.receipt = Some(Receipt::Committed);
            state.verified_approval = true;
            state.status_before = status_lines(observation);
            return;
        }
        // After the fact: a step Rust read as harmless made the page say it
        // committed something.
        let before = std::mem::take(&mut state.status_before);
        if let Some(line) = status_lines(observation)
            .into_iter()
            .find(|line| !before.contains(line) && says_done(line))
        {
            state.unconfirmed = Some(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page(mut nodes: Vec<serde_json::Value>) -> SemanticObservation {
        for node in &mut nodes {
            if node
                .get("o")
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|ops| ops & 15 != 0)
            {
                node["ai"] = node["k"].clone();
            }
        }
        super::super::tests::reading_observation_with_approval(
            json!(nodes),
            "complete",
            Some("000102030405060708090a0b0c0d0e0f"),
        )
    }

    #[test]
    fn an_account_task_can_request_an_unfamiliar_sign_in_without_spending_refusal_turns() {
        let unfamiliar = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"heading","n":"Continue to your team","l":1}),
            json!({"k":3,"p":0,"r":"button","n":"Continue","o":1,"ak":1}),
        ]);
        let account = SiteWorkPolicy {
            gate: Arc::new(SiteGate::default().expecting_account(true)),
            asks: true,
        };
        assert!(!account.sign_in_optional(&unfamiliar));
        let public = SiteWorkPolicy {
            gate: Arc::new(SiteGate::default()),
            asks: true,
        };
        assert!(public.sign_in_optional(&unfamiliar));
    }

    #[test]
    fn research_readers_raise_visible_sign_in_and_check_walls_before_extraction() {
        let sign_in = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"password","n":"Password","q":"secret","o":2,"b":{"x":1,"y":1,"w":200,"h":30}}),
            json!({"k":3,"p":0,"r":"heading","n":"Sign in to your account","l":1}),
        ]);
        let check = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"frame_boundary","n":"hCaptcha challenge","b":{"x":1,"y":1,"w":300,"h":90}}),
        ]);
        let results = super::super::results_page::ResultsPagePolicy {
            reader: Arc::new(super::super::results_page::ResultsReader::new(
                zephium_core::search::SearchEngine::Google,
                None,
            )),
        };
        for (look, reason) in [
            (sign_in, AgentBrowserHumanReason::SignIn),
            (check, AgentBrowserHumanReason::HumanChallenge),
        ] {
            assert_eq!(
                super::super::read_interactions::ReadingInteractionPolicy.human_wall(&look),
                Some(reason)
            );
            assert_eq!(
                super::super::read_interactions::SessionReadingPolicy.human_wall(&look),
                Some(reason)
            );
            assert_eq!(results.human_wall(&look), Some(reason));
        }
    }

    #[test]
    fn an_account_view_can_request_sign_in_without_a_password_form() {
        let picker = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"heading","n":"Find your workspace","l":1}),
            json!({"k":3,"p":0,"r":"textbox","n":"Workspace URL","o":2}),
        ]);
        let gate = Arc::new(
            SiteGate::new("slack.com".into(), false, false)
                .reading_view("https://slack.com", "Read my messages"),
        );
        let policy = SiteWorkPolicy { gate, asks: true };
        assert!(!policy.sign_in_optional(&picker));
        let public = SiteWorkPolicy {
            gate: Arc::new(SiteGate::default()),
            asks: true,
        };
        assert!(
            !public.sign_in_optional(&picker),
            "a credential-free workspace chooser is still a wall"
        );
        assert_eq!(
            public.human_wall(&picker),
            Some(AgentBrowserHumanReason::SignIn)
        );
        assert_eq!(
            super::super::read_interactions::ReadingInteractionPolicy.human_wall(&picker),
            Some(AgentBrowserHumanReason::SignIn)
        );
    }

    fn effect(observation: &SemanticObservation, key: u64, kind: SemanticActionKind) -> SiteEffect {
        effect_key(observation, key, kind, None)
    }
    fn effect_key(
        observation: &SemanticObservation,
        key: u64,
        kind: SemanticActionKind,
        press: Option<SemanticPressKey>,
    ) -> SiteEffect {
        let snapshot = observation.frames().first().unwrap();
        let node = &snapshot.nodes()[usize::try_from(key - 1).unwrap()];
        classify(kind, press, node, snapshot)
    }

    #[test]
    fn committing_controls_are_held_and_drafting_proceeds() {
        let slack = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"textbox","n":"Message #design","o":10}),
            json!({"k":3,"p":0,"r":"button","n":"Send now","o":1,"ak":1}),
            json!({"k":4,"p":0,"r":"button","n":"Emoji","o":1,"ak":1}),
            json!({"k":5,"p":0,"r":"link","n":"#general","o":1,"ak":4}),
            json!({"k":6,"p":0,"r":"tab","n":"Threads","o":1}),
        ]);
        assert_eq!(
            effect(&slack, 3, SemanticActionKind::Click),
            SiteEffect::Commit(Consequence::Communication)
        );
        assert_eq!(
            effect(&slack, 4, SemanticActionKind::Click),
            SiteEffect::Draft
        );
        assert_eq!(
            effect(&slack, 5, SemanticActionKind::Click),
            SiteEffect::Read
        );
        assert_eq!(
            effect(&slack, 6, SemanticActionKind::Click),
            SiteEffect::Read
        );
        assert_eq!(
            effect(&slack, 2, SemanticActionKind::Fill),
            SiteEffect::Draft
        );
        assert_eq!(
            effect_key(
                &slack,
                2,
                SemanticActionKind::Press,
                Some(SemanticPressKey::Enter)
            ),
            SiteEffect::Commit(Consequence::Communication)
        );
        let booking = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"button","n":"Request to book","o":1,"ak":1}),
            json!({"k":3,"p":0,"r":"button","n":"Apply filters","o":1,"ak":1}),
            json!({"k":4,"p":0,"r":"dialog","n":"Delete this page?"}),
            json!({"k":5,"p":3,"r":"paragraph","t":"This cannot be undone."}),
            json!({"k":6,"p":3,"r":"button","n":"Yes","o":1,"ak":1}),
            json!({"k":7,"p":3,"r":"button","n":"Cancel","o":1,"ak":1}),
        ]);
        assert_eq!(
            effect(&booking, 2, SemanticActionKind::Click),
            SiteEffect::Commit(Consequence::Purchase)
        );
        assert_eq!(
            effect(&booking, 3, SemanticActionKind::Click),
            SiteEffect::Draft
        );
        assert_eq!(
            effect(&booking, 6, SemanticActionKind::Click),
            SiteEffect::Commit(Consequence::Destructive)
        );
        assert_eq!(
            effect(&booking, 7, SemanticActionKind::Click),
            SiteEffect::Read
        );
        let search = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"landmark","lm":"search"}),
            json!({"k":3,"p":1,"r":"searchbox","n":"Where","o":2}),
            json!({"k":4,"p":1,"r":"button","n":"Search","o":1,"ak":2}),
        ]);
        assert_eq!(
            effect(&search, 3, SemanticActionKind::Fill),
            SiteEffect::Read
        );
        assert_eq!(
            effect(&search, 4, SemanticActionKind::Click),
            SiteEffect::Read
        );
        assert_eq!(
            effect_key(
                &search,
                3,
                SemanticActionKind::Press,
                Some(SemanticPressKey::Enter)
            ),
            SiteEffect::Read
        );
    }

    #[test]
    fn only_fields_that_commit_nothing_take_a_rewritten_value_as_feedback() {
        let page = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"landmark","lm":"search"}),
            json!({"k":3,"p":1,"r":"combobox","n":"From","o":2}),
            json!({"k":4,"p":0,"r":"textbox","n":"Check-in date","o":2}),
            json!({"k":5,"p":0,"r":"textbox","n":"Card number","o":2,"q":"sensitive"}),
            json!({"k":6,"p":0,"r":"password","n":"Password","o":2}),
            json!({"k":7,"p":0,"r":"textbox","n":"Email","o":2,"ff":6}),
            json!({"k":8,"p":0,"r":"textbox","n":"Search products","o":2,"ff":5}),
        ]);
        let snapshot = page.frames().first().unwrap();
        let field = |key: u64, kind| {
            rewritable(
                kind,
                &snapshot.nodes()[usize::try_from(key - 1).unwrap()],
                snapshot,
            )
        };
        assert!(field(3, SemanticActionKind::Fill), "a place picker");
        assert!(field(4, SemanticActionKind::Fill), "a field with no form");
        assert!(
            field(8, SemanticActionKind::Fill),
            "a form that only queries"
        );
        assert!(
            !field(3, SemanticActionKind::Click),
            "only typing or choosing"
        );
        assert!(!field(5, SemanticActionKind::Fill), "a sensitive field");
        assert!(!field(6, SemanticActionKind::Fill), "a password");
        assert!(!field(7, SemanticActionKind::Fill), "a form that posts");
    }

    #[test]
    fn rich_editors_draft_until_their_own_control_commits() {
        // Slack's composer with its Send button out of the observation, and
        // Linear's new-issue dialog of rich editors with its Create button.
        let apps = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"textbox","n":"Message to design","o":11,"fs":1,"es":[1,2,false]}),
            json!({"k":3,"p":0,"r":"button","n":"Create new issue","o":1,"ak":1}),
            json!({"k":4,"p":0,"r":"dialog","n":"New issue"}),
            json!({"k":5,"p":3,"r":"textbox","n":"Issue title","o":11,"fs":1,"es":[1,2,false]}),
            json!({"k":6,"p":3,"r":"textbox","n":"Add description…","o":11,"fs":1,"es":[1,2,false]}),
            json!({"k":7,"p":3,"r":"button","n":"Team: Backlog","o":1,"ak":1}),
            json!({"k":8,"p":3,"r":"button","n":"Create issue","o":1,"ak":1}),
            json!({"k":9,"p":0,"r":"textbox","n":"Block 1","o":11,"fs":1,"es":[1,2,false]}),
        ]);
        let tasks = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"button","n":"Delete task","o":1,"ak":1}),
            json!({"k":3,"p":0,"r":"group","n":"Quick add"}),
            json!({"k":4,"p":2,"r":"textbox","n":"Task name","o":11,"fs":1,"es":[1,2,false]}),
            json!({"k":5,"p":2,"r":"button","n":"Remove date","o":1,"ak":1}),
            json!({"k":6,"p":2,"r":"button","n":"Add task","o":1,"ak":1}),
        ]);
        assert_eq!(
            effect(&tasks, 4, SemanticActionKind::Fill),
            SiteEffect::Draft,
            "an inline editor drafts until its own Add button",
        );
        assert_eq!(
            effect_key(
                &tasks,
                4,
                SemanticActionKind::Press,
                Some(SemanticPressKey::Enter)
            ),
            SiteEffect::Commit(Consequence::Save),
            "Enter makes the drafted task, whatever else the editor offers to remove",
        );
        assert_eq!(
            effect(&apps, 2, SemanticActionKind::Fill),
            SiteEffect::Draft
        );
        assert_eq!(
            effect_key(
                &apps,
                2,
                SemanticActionKind::Press,
                Some(SemanticPressKey::Enter),
            ),
            SiteEffect::Commit(Consequence::Communication),
            "a named composer still sends when its Send button is outside the look",
        );
        assert_eq!(
            effect(&apps, 3, SemanticActionKind::Click),
            SiteEffect::Draft
        );
        assert_eq!(
            effect(&apps, 5, SemanticActionKind::Fill),
            SiteEffect::Draft
        );
        assert_eq!(
            effect(&apps, 6, SemanticActionKind::Fill),
            SiteEffect::Draft
        );
        assert_eq!(
            effect(&apps, 7, SemanticActionKind::Click),
            SiteEffect::Draft
        );
        assert_eq!(
            effect(&apps, 8, SemanticActionKind::Click),
            SiteEffect::Commit(Consequence::Save)
        );
        // A page of blocks still saves as it is typed.
        assert_eq!(
            effect(&apps, 9, SemanticActionKind::Fill),
            SiteEffect::Commit(Consequence::Edit)
        );
    }

    #[test]
    fn form_facts_let_queries_through_and_hold_posts() {
        let forms = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"textbox","n":"Find","o":10,"ff":13}),
            json!({"k":3,"p":0,"r":"button","n":"Go","o":1,"ak":2,"ff":13}),
            json!({"k":4,"p":0,"r":"button","n":"Apply","o":1,"ak":2,"ff":5}),
            json!({"k":5,"p":0,"r":"button","n":"Continue","o":1,"ak":2,"ff":6}),
            json!({"k":6,"p":0,"r":"button","n":"Go","o":1,"ak":2,"ff":1}),
        ]);
        assert_eq!(
            effect(&forms, 2, SemanticActionKind::Fill),
            SiteEffect::Read
        );
        assert_eq!(
            effect_key(
                &forms,
                2,
                SemanticActionKind::Press,
                Some(SemanticPressKey::Enter)
            ),
            SiteEffect::Read
        );
        assert_eq!(
            effect(&forms, 3, SemanticActionKind::Click),
            SiteEffect::Read
        );
        assert_eq!(
            effect(&forms, 4, SemanticActionKind::Click),
            SiteEffect::Read
        );
        assert_eq!(
            effect(&forms, 5, SemanticActionKind::Click),
            SiteEffect::Commit(Consequence::Save)
        );
        // A GET to another origin is not the page's own query.
        assert_eq!(
            effect(&forms, 6, SemanticActionKind::Click),
            SiteEffect::Commit(Consequence::Save)
        );
    }

    #[test]
    fn a_field_named_for_search_searches_and_enter_on_a_control_is_its_click() {
        let shop = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"landmark","lm":"banner"}),
            json!({"k":3,"p":1,"r":"textbox","n":"Czego szukasz?","o":10}),
            json!({"k":4,"p":1,"r":"button","n":"Zamknij powiadomienie","o":9,"ak":1}),
            json!({"k":5,"p":1,"r":"link","n":"Laptopy","o":9,"u":"https://example.test/laptopy"}),
            json!({"k":6,"p":1,"r":"textbox","n":"Message","o":10}),
            json!({"k":7,"p":1,"r":"button","n":"Delete account","o":9,"ak":1}),
        ]);
        let enter = |key: u64| {
            effect_key(
                &shop,
                key,
                SemanticActionKind::Press,
                Some(SemanticPressKey::Enter),
            )
        };
        assert_eq!(enter(3), SiteEffect::Read);
        assert_eq!(enter(4), SiteEffect::Read);
        assert_eq!(enter(5), SiteEffect::Read);
        assert_eq!(enter(7), SiteEffect::Commit(Consequence::Destructive));
        assert!(matches!(enter(6), SiteEffect::Commit(_)));
    }

    #[test]
    fn putting_something_in_a_cart_saves_and_placing_the_order_buys() {
        assert_eq!(commit_words(" add to cart "), Some(Consequence::Save));
        assert_eq!(commit_words(" dodaj do koszyka "), Some(Consequence::Save));
        assert_eq!(commit_words(" place order "), Some(Consequence::Purchase));
        assert_eq!(commit_words(" buy now "), Some(Consequence::Purchase));
    }

    #[test]
    fn an_item_already_added_to_the_cart_is_not_added_again() {
        let product = |name: &str| {
            page(vec![
                json!({"k":1,"r":"document","o":16}),
                json!({"k":2,"p":0,"r":"heading","l":1,"n":name}),
                json!({"k":3,"p":0,"r":"button","n":"Add to cart","o":1,"ak":1,"b":{"x":1,"y":1,"w":80,"h":30}}),
            ])
        };
        let policy = SiteWorkPolicy {
            gate: Arc::new(SiteGate::new("shop.example".into(), true, false)),
            asks: true,
        };
        let pliers = product("Combination Pliers");
        let add = |look: &SemanticObservation| {
            policy.assess(&click(look, 3, SemanticEffectClass::ExternalWrite), look)
        };
        assert!(add(&pliers).is_ok());
        assert_eq!(add(&pliers).err(), Some(AgentWorkFailure::AlreadyDone));
        // Another product's button is another item.
        assert!(add(&product("Slip Joint Pliers")).is_ok());
    }

    #[test]
    fn message_names_do_not_lower_purchase_or_destructive_enter_effects() {
        for (control, consequence) in [
            ("Pay", Consequence::Purchase),
            ("Delete", Consequence::Destructive),
        ] {
            let look = page(vec![
                json!({"k":1,"r":"document","o":16}),
                json!({"k":2,"p":0,"r":"textbox","n":"Message","o":10}),
                json!({"k":3,"p":0,"r":"button","n":control,"o":1}),
            ]);
            assert_eq!(
                effect_key(
                    &look,
                    2,
                    SemanticActionKind::Press,
                    Some(SemanticPressKey::Enter)
                ),
                SiteEffect::Commit(consequence),
            );
        }
    }

    fn booking(total: &str) -> SemanticObservation {
        page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"landmark","lm":"form","n":"Reserve","fc":true}),
            json!({"k":3,"p":1,"r":"heading","l":2,"n":"Cabin by the lake","fc":true}),
            json!({"k":4,"p":1,"r":"paragraph","t":"Dates: 3–5 May","fc":true}),
            json!({"k":5,"p":1,"r":"paragraph","t":format!("Total: {total}"),"fc":true}),
            json!({"k":6,"p":1,"r":"button","n":"Request to book","o":9,"ak":2,"fc":true,
                "b":{"x":1,"y":1,"w":100,"h":30}}),
        ])
    }

    fn click(
        observation: &SemanticObservation,
        key: usize,
        effect: SemanticEffectClass,
    ) -> SemanticPreparedAction {
        let snapshot = &observation.frames()[0];
        let proposal = SemanticActionProposal::try_new(
            SemanticActionIntent::Click {
                target: snapshot.nodes()[key - 1].reference(),
            },
            effect,
            SemanticWaitCondition::Immediate,
            SemanticVerification::PageChanged,
            SemanticSettleBudget::try_new(2000).unwrap(),
        )
        .unwrap();
        SemanticActionBatch::bind(
            SemanticActionBatchId::new(1).unwrap(),
            observation,
            &[snapshot.frame().clone()],
            vec![proposal],
        )
        .unwrap()
        .actions()[0]
            .prepare(snapshot)
            .unwrap()
    }

    #[test]
    fn a_compact_slack_composer_enter_holds_its_own_draft_and_requires_exact_approval() {
        let look = |draft: &str| {
            page(vec![
                json!({"k":1,"r":"document","o":16,"fc":true}),
                json!({"k":2,"p":0,"r":"searchbox","n":"Search Slack","o":10,"fc":true,
                "v":{"k":"text","value":"older search"},"b":{"x":1,"y":1,"w":200,"h":30}}),
                json!({"k":3,"p":0,"r":"heading","l":1,"n":"#design","fc":true}),
                // A viewport can omit Send while retaining Slack's rich composer.
                json!({"k":4,"p":0,"r":"textbox","n":"Message to design","o":11,"fs":1,"es":[1,2,false],"fc":true,
                "v":{"k":"text","value":draft},"b":{"x":1,"y":100,"w":400,"h":60}}),
                json!({"k":5,"p":0,"r":"button","n":"Save for later","o":1,"fc":true}),
            ])
        };
        let enter = |observation: &SemanticObservation| {
            let snapshot = &observation.frames()[0];
            let proposal = SemanticActionProposal::try_new(
                SemanticActionIntent::Press {
                    target: snapshot.nodes()[3].reference(),
                    key: SemanticPressKey::Enter,
                },
                SemanticEffectClass::Communication,
                SemanticWaitCondition::Immediate,
                SemanticVerification::PageChanged,
                SemanticSettleBudget::try_new(2000).unwrap(),
            )
            .unwrap();
            SemanticActionBatch::bind(
                SemanticActionBatchId::new(1).unwrap(),
                observation,
                &[snapshot.frame().clone()],
                vec![proposal],
            )
            .unwrap()
            .actions()[0]
                .prepare(snapshot)
                .unwrap()
        };
        let gate = Arc::new(SiteGate::new("slack.com".into(), false, false));
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        let original = look("testing agentic browsing on Windows");
        assert!(matches!(
            policy.assess(&enter(&original), &original),
            Err(AgentWorkFailure::ActionDenied)
        ));
        let pending = gate.pending().unwrap().preview;
        assert_eq!(pending.consequence, Consequence::Communication);
        assert_eq!(
            pending.text.as_deref(),
            Some("testing agentic browsing on Windows")
        );
        gate.approve(false).unwrap();
        // A changed draft consumes the old approval without permitting Enter.
        let changed = look("changed draft");
        assert!(matches!(
            policy.assess(&enter(&changed), &changed),
            Err(AgentWorkFailure::ActionDenied)
        ));
        assert_eq!(gate.take_receipt(), Some(Receipt::NotSent));
        assert_eq!(
            gate.pending().unwrap().preview.text.as_deref(),
            Some("changed draft")
        );
        gate.approve(false).unwrap();
        assert!(policy.assess(&enter(&changed), &changed).is_ok());
        assert!(matches!(
            policy.assess(&enter(&changed), &changed),
            Err(AgentWorkFailure::ActionDenied)
        ));
        assert_eq!(gate.finish(), Some(Receipt::Unverified));
    }

    #[test]
    fn a_commit_is_held_with_a_page_preview_and_runs_once_only_while_unchanged() {
        let gate = Arc::new(SiteGate::new("airbnb.com".into(), false, false));
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        let page = booking("$1,240.00");
        // Declared as a mere read, the page still shows a purchase.
        let action = click(&page, 6, SemanticEffectClass::Read);
        assert!(matches!(
            policy.assess(&action, &page),
            Err(AgentWorkFailure::ActionDenied)
        ));
        assert_eq!(
            policy.human_wall(&page),
            Some(AgentBrowserHumanReason::UserDecision)
        );
        let held = gate.pending().unwrap().preview;
        assert_eq!(held.consequence, Consequence::Purchase);
        assert_eq!(held.headline, "Request to book for $1,240.00?");
        assert_eq!(held.action, "press Request to book");
        assert!(held
            .facts
            .contains(&("Dates".to_owned(), "3–5 May".to_owned())));
        gate.approve(false).unwrap();
        assert_eq!(gate.approved_class(), Some(SemanticEffectClass::Purchase));
        // The successor must declare the true effect; then it runs once.
        assert!(matches!(
            policy.assess(&action, &page),
            Err(AgentWorkFailure::EffectRequired(
                SemanticEffectClass::Purchase
            ))
        ));
        let purchase = click(&page, 6, SemanticEffectClass::Purchase);
        assert!(policy.assess(&purchase, &page).is_ok());
        assert!(matches!(
            policy.assess(&purchase, &page),
            Err(AgentWorkFailure::ActionDenied)
        ));
        assert_eq!(gate.finish(), Some(Receipt::Unverified));

        // A changed total spends the approval and asks again.
        let gate = Arc::new(SiteGate::new("airbnb.com".into(), false, false));
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        let _ = policy.assess(&click(&page, 6, SemanticEffectClass::Purchase), &page);
        gate.approve(false).unwrap();
        let changed = booking("$1,480.00");
        assert!(matches!(
            policy.assess(&click(&changed, 6, SemanticEffectClass::Purchase), &changed),
            Err(AgentWorkFailure::ActionDenied)
        ));
        assert_eq!(gate.take_receipt(), Some(Receipt::NotSent));
        assert_eq!(
            gate.pending().unwrap().preview.headline,
            "Request to book for $1,480.00?"
        );
        // Declined, the same step is refused without asking again.
        gate.decline().unwrap();
        assert!(matches!(
            policy.assess(&click(&changed, 6, SemanticEffectClass::Purchase), &changed),
            Err(AgentWorkFailure::ActionDenied)
        ));
        assert!(gate.pending().is_none());
    }

    #[test]
    fn approved_action_history_survives_consuming_the_receipt() {
        let gate = Arc::new(SiteGate::new("example.com".into(), false, false));
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        let observation = booking("$1,240.00");
        let action = click(&observation, 6, SemanticEffectClass::Purchase);
        assert!(!gate.has_verified_approval());
        assert!(policy.assess(&action, &observation).is_err());
        gate.approve(false).unwrap();
        assert!(policy.assess(&action, &observation).is_ok());
        assert!(!gate.has_verified_approval());
        policy.after_step(&observation);
        assert_eq!(gate.take_receipt(), Some(Receipt::Committed));
        assert_eq!(gate.take_receipt(), None);
        assert!(gate.has_verified_approval());
    }

    #[test]
    fn typing_on_a_site_the_person_did_not_name_waits_until_allowed_for_the_run() {
        let search = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"landmark","lm":"search","fc":true}),
            json!({"k":3,"p":1,"r":"searchbox","n":"Search","o":2,"fc":true,
                "v":{"k":"text","value":""},"b":{"x":1,"y":1,"w":200,"h":30}}),
        ]);
        let fill = |effect| {
            let snapshot = &search.frames()[0];
            let proposal = SemanticActionProposal::try_new(
                SemanticActionIntent::Fill {
                    target: snapshot.nodes()[2].reference(),
                    value: SemanticActionText::try_new("aisle seat WAW-SFO".into()).unwrap(),
                },
                effect,
                SemanticWaitCondition::Immediate,
                SemanticVerification::TargetValueMatchesInput,
                SemanticSettleBudget::try_new(2000).unwrap(),
            )
            .unwrap();
            SemanticActionBatch::bind(
                SemanticActionBatchId::new(1).unwrap(),
                &search,
                &[snapshot.frame().clone()],
                vec![proposal],
            )
            .unwrap()
            .actions()[0]
                .prepare(snapshot)
                .unwrap()
        };
        // Without the hold a search is a read, as before.
        let open = SiteWorkPolicy {
            gate: Arc::new(SiteGate::new("example.com".into(), false, false)),
            asks: true,
        };
        assert!(open
            .assess(&fill(SemanticEffectClass::Read), &search)
            .is_ok());

        let gate = Arc::new(SiteGate::new("example.com".into(), false, false).holding_typing(true));
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        assert!(policy
            .assess(&fill(SemanticEffectClass::Read), &search)
            .is_err());
        let held = gate.pending().unwrap().preview;
        assert_eq!(held.consequence, Consequence::Type);
        assert_eq!(held.headline, "Type on example.com?");
        assert_eq!(held.text.as_deref(), Some("aisle seat WAW-SFO"));
        gate.approve(true).unwrap();
        assert!(!gate.allow_edits());
        // The approved fill runs as declared, and later typing needs no question.
        assert!(policy
            .assess(&fill(SemanticEffectClass::Read), &search)
            .is_ok());
        assert!(policy
            .assess(&fill(SemanticEffectClass::LocalWrite), &search)
            .is_ok());
        assert!(gate.pending().is_none());
    }

    #[test]
    fn an_autosaving_editor_asks_once_then_edits_run_for_the_run() {
        let doc = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"heading","l":1,"n":"Meeting notes","fc":true}),
            json!({"k":3,"p":0,"r":"textbox","n":"Notes","o":3,"fs":1,"es":[1,1,false],"fc":true,
                "v":{"k":"text","value":""},"b":{"x":1,"y":40,"w":400,"h":300}}),
        ]);
        let fill = |effect| {
            let snapshot = &doc.frames()[0];
            let proposal = SemanticActionProposal::try_new(
                SemanticActionIntent::Fill {
                    target: snapshot.nodes()[2].reference(),
                    value: SemanticActionText::try_new("Agenda: launch".into()).unwrap(),
                },
                effect,
                SemanticWaitCondition::Immediate,
                SemanticVerification::TargetValueMatchesInput,
                SemanticSettleBudget::try_new(2000).unwrap(),
            )
            .unwrap();
            SemanticActionBatch::bind(
                SemanticActionBatchId::new(1).unwrap(),
                &doc,
                &[snapshot.frame().clone()],
                vec![proposal],
            )
            .unwrap()
            .actions()[0]
                .prepare(snapshot)
                .unwrap()
        };
        let gate = Arc::new(SiteGate::new("notion.so".into(), false, false));
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        assert!(policy
            .assess(&fill(SemanticEffectClass::LocalWrite), &doc)
            .is_err());
        let held = gate.pending().unwrap().preview;
        assert_eq!(held.consequence, Consequence::Edit);
        assert_eq!(held.text.as_deref(), Some("Agenda: launch"));
        assert_eq!(held.headline, "Save changes on notion.so?");
        gate.approve(true).unwrap();
        assert!(gate.allow_edits());
        assert!(policy
            .assess(&fill(SemanticEffectClass::ExternalWrite), &doc)
            .is_ok());
        // Later edits need no question.
        assert!(policy
            .assess(&fill(SemanticEffectClass::ExternalWrite), &doc)
            .is_ok());
        assert!(gate.pending().is_none());
    }

    #[test]
    fn a_step_read_as_harmless_that_says_it_committed_stops_the_page() {
        let before = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"button","n":"Done","o":9,"ak":1,"fc":true,
                "b":{"x":1,"y":1,"w":100,"h":30}}),
        ]);
        let after = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":3,"p":0,"r":"status","t":"Message sent","fc":true}),
        ]);
        let gate = Arc::new(SiteGate::new("slack.com".into(), false, false));
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        assert!(policy
            .assess(&click(&before, 2, SemanticEffectClass::LocalWrite), &before)
            .is_ok());
        policy.after_step(&after);
        assert_eq!(gate.unconfirmed().as_deref(), Some("Message sent"));
        for (line, done) in [
            ("Message sent", true),
            ("Booking confirmed", true),
            ("Your order has been placed and paid", true),
            ("Thank you for your order", true),
            (
                "Most booked hotels in Krakow and surrounding area in the past month",
                false,
            ),
            ("Recently ordered together", false),
            ("Sent", true),
            ("Draft saved", false),
            ("Compose: Draft saved", false),
        ] {
            assert_eq!(says_done(line), done, "{line}");
        }
        assert_eq!(
            policy.human_wall(&after),
            Some(AgentBrowserHumanReason::Verification)
        );
    }

    #[test]
    fn a_stays_checkout_date_is_no_purchase() {
        let widget = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"button","n":"Change dates; Check-in: Add date; Checkout: Add date","o":1,"ak":1}),
            json!({"k":3,"p":0,"r":"button","n":"Reserve","o":1,"ak":1}),
        ]);
        assert_eq!(
            effect(&widget, 2, SemanticActionKind::Click),
            SiteEffect::Draft
        );
        assert_eq!(
            effect(&widget, 3, SemanticActionKind::Click),
            SiteEffect::Commit(Consequence::Purchase)
        );
    }

    #[test]
    fn a_cookie_banner_choice_is_a_read_and_never_held() {
        let banner = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"group","fc":true}),
            json!({"k":3,"p":1,"r":"paragraph","t":"We use cookies to personalise content.","fc":true}),
            json!({"k":4,"p":1,"r":"button","n":"Reject All","o":1,"ak":1,"fc":true,
                "b":{"x":1,"y":1,"w":100,"h":30}}),
            json!({"k":5,"p":1,"r":"button","n":"Accept All","o":1,"ak":1,"fc":true,
                "b":{"x":120,"y":1,"w":100,"h":30}}),
        ]);
        assert_eq!(
            effect(&banner, 4, SemanticActionKind::Click),
            SiteEffect::Read
        );
        let gate = Arc::new(SiteGate::new("lego.com".into(), false, false));
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        assert!(policy
            .assess(&click(&banner, 4, SemanticEffectClass::Read), &banner)
            .is_ok());
        // Declared as a message, it is asked for as the read it is.
        assert!(matches!(
            policy.assess(
                &click(&banner, 4, SemanticEffectClass::Communication),
                &banner
            ),
            Err(AgentWorkFailure::EffectRequired(SemanticEffectClass::Read))
        ));
        assert!(matches!(
            policy.assess(&click(&banner, 5, SemanticEffectClass::Read), &banner),
            Err(AgentWorkFailure::ActionDenied)
        ));
        assert!(gate.pending().is_none() && !gate.held_back());
        assert_eq!(
            policy.consent_dismissal(&banner),
            Some(banner.frames()[0].nodes()[3].reference())
        );
    }

    #[test]
    fn a_code_card_or_challenge_the_agent_cannot_pass_goes_to_the_person() {
        let policy = SiteWorkPolicy {
            gate: Arc::new(SiteGate::new("github.com".into(), false, false)),
            asks: true,
        };
        let at = json!({"x":10,"y":50,"w":200,"h":30});
        // A two-factor page names no sign-in phrase; its code field is secret.
        let two_factor = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"heading","n":"Two-factor authentication","l":1}),
            json!({"k":3,"p":0,"r":"textbox","n":"Authentication code","q":"secret","o":11,"b":at}),
            json!({"k":4,"p":0,"r":"button","n":"Verify","o":1,"ak":1}),
        ]);
        assert!(!sign_in_wall(&two_factor));
        assert_eq!(
            policy.human_wall(&two_factor),
            Some(AgentBrowserHumanReason::SignIn)
        );
        let checkout = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"heading","n":"Payment","l":2}),
            json!({"k":3,"p":0,"r":"textbox","n":"Card number","q":"secret","o":11,"b":at}),
            json!({"k":4,"p":0,"r":"button","n":"Place order","o":1,"ak":1}),
        ]);
        assert_eq!(
            policy.human_wall(&checkout),
            Some(AgentBrowserHumanReason::SensitiveEffect)
        );
        // Hidden or read-only secret fields hold nothing.
        for field in [
            json!({"k":3,"p":0,"r":"textbox","n":"Card number","q":"secret","o":11}),
            json!({"k":3,"p":0,"r":"textbox","n":"Card number","q":"secret","o":9,"b":at}),
        ] {
            let quiet = page(vec![
                json!({"k":1,"r":"document","o":16}),
                json!({"k":2,"p":0,"r":"heading","n":"Payment","l":2}),
                field,
            ]);
            assert_eq!(policy.human_wall(&quiet), None);
        }
        let challenge = |title: &str, width: u32| {
            page(vec![
                json!({"k":1,"r":"document","o":16}),
                json!({"k":2,"p":0,"r":"textbox","n":"Email","o":11,"b":at}),
                json!({"k":3,"p":0,"r":"frame_boundary","n":title,"b":{"x":10,"y":90,"w":width,"h":78}}),
            ])
        };
        for (title, width) in [
            (
                "Widget containing checkbox for hCaptcha security challenge",
                300,
            ),
            ("Widget containing a Cloudflare security challenge", 300),
            ("reCAPTCHA", 304),
        ] {
            assert_eq!(
                policy.human_wall(&challenge(title, width)),
                Some(AgentBrowserHumanReason::HumanChallenge),
                "{title}"
            );
        }
        for title in [
            "Secure card payment input frame",
            "Secure Credit Card Frame - Credit Card Number",
            "Iframe for card number",
        ] {
            assert_eq!(
                policy.human_wall(&challenge(title, 300)),
                Some(AgentBrowserHumanReason::SensitiveEffect),
                "{title}"
            );
        }
        // reCAPTCHA's score badge shares the title and stands on many pages.
        assert_eq!(policy.human_wall(&challenge("reCAPTCHA", 256)), None);
        assert_eq!(policy.human_wall(&challenge("Embedded video", 640)), None);
    }

    #[test]
    fn visible_password_examples_and_settings_do_not_block_public_reading() {
        for title in ["Web form", "Form controls", "Account settings"] {
            let observation = page(vec![
                json!({"k":1,"r":"document","o":16}),
                json!({"k":2,"p":0,"r":"heading","n":title,"l":1}),
                json!({"k":3,"p":0,"r":"password","n":"Password","q":"secret","o":2,"b":{"x":10,"y":10,"w":200,"h":30}}),
                json!({"k":4,"p":0,"r":"button","n":"Submit","o":1,"ak":1}),
            ]);
            assert!(!sign_in_wall(&observation), "{title}");
            assert_eq!(reading_wall(&observation), None, "{title}");
            let password = observation.frames()[0]
                .nodes()
                .iter()
                .find(|node| node.role() == SemanticRole::Password)
                .unwrap();
            assert_eq!(password.sensitivity(), SemanticSensitivity::Secret);
        }
    }

    #[test]
    fn workspace_identity_entry_with_sign_in_intent_is_a_wall() {
        for name in ["Workspace URL", "Organization name", "Tenant ID"] {
            let nodes = vec![
                json!({"k":1,"r":"document","o":16}),
                json!({"k":2,"p":0,"r":"heading","n":"Sign in to your workspace","l":1}),
                json!({"k":3,"p":0,"r":"textbox","n":name,"o":2}),
                json!({"k":4,"p":0,"r":"button","n":"Continue","o":1,"ak":1}),
            ];
            assert!(sign_in_wall(&page(nodes.clone())), "{name}");
            let mut search = nodes;
            search[2]["r"] = json!("searchbox");
            assert!(!sign_in_wall(&page(search)), "search is not identity entry");
        }
    }

    #[test]
    fn a_password_requires_sign_in_context_and_small_identity_forms_are_walls() {
        assert!(!sign_in_wall(&page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"password","n":"Password","q":"secret","o":2,"b":{"x":10,"y":10,"w":200,"h":30}}),
        ])));
        assert!(sign_in_wall(&page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"heading","n":"Sign in to your account","l":1}),
            json!({"k":3,"p":0,"r":"password","n":"Password","q":"secret","o":2}),
        ])));
        assert!(!sign_in_wall(&page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"password","n":"Password","q":"secret","o":2}),
        ])));
        assert!(sign_in_wall(&page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"heading","n":"Sign in to Notion","l":1}),
            json!({"k":3,"p":0,"r":"textbox","n":"Email","q":"sensitive","o":2,"b":{"x":10,"y":50,"w":200,"h":30}}),
        ])));
        // GitHub's open landing page: a sign-up email box beside its search.
        assert!(!sign_in_wall(&page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"landmark","lm":"search"}),
            json!({"k":3,"p":0,"r":"button","n":"Sign in","o":1,"ak":1}),
            json!({"k":4,"p":0,"r":"textbox","n":"Email address","q":"sensitive","o":2,"b":{"x":10,"y":50,"w":200,"h":30}}),
        ])));
        assert!(!sign_in_wall(&page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"heading","n":"Inbox","l":1}),
            json!({"k":3,"p":0,"r":"button","n":"Log in to another account","o":1,"ak":1}),
        ])));
        // Skyscanner's sign-in offer over its flight search closes.
        let offer = |close: &str| {
            let mut nodes = vec![
                json!({"k":1,"r":"document","o":16}),
                json!({"k":2,"p":0,"r":"button","n":"Zaloguj się","o":1,"ak":1}),
                json!({"k":3,"p":0,"r":"dialog","n":"Zaloguj się lub utwórz konto"}),
                json!({"k":4,"p":2,"r":"button","n":close,"o":1,"ak":1}),
                json!({"k":5,"p":2,"r":"textbox","n":"Adres e-mail","q":"sensitive","o":2,"b":{"x":10,"y":50,"w":200,"h":30}}),
            ];
            nodes.extend((6..30).map(|k| json!({"k":k,"p":0,"r":"paragraph","t":"Loty"})));
            page(nodes)
        };
        assert!(!sign_in_wall(&offer("Zamknij okno dialogowe")));
        assert!(sign_in_wall(&offer("Dalej")));
    }

    #[test]
    fn a_task_for_the_persons_account_hands_over_on_a_landing_page_that_offers_sign_in() {
        let landing = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"landmark","lm":"banner"}),
            json!({"k":3,"p":1,"r":"link","n":"Pricing","o":9,"u":"https://example.test/pricing"}),
            json!({"k":4,"p":1,"r":"link","n":"Log in","o":9,"u":"https://example.test/login"}),
            json!({"k":5,"p":0,"r":"heading","n":"Notes and docs in one place","l":1}),
        ]);
        let policy = |view: bool, account: bool| SiteWorkPolicy {
            gate: Arc::new({
                let gate =
                    SiteGate::new("example.test".into(), false, false).expecting_account(account);
                if view {
                    gate.reading_view("https://example.test/", "my recent pages")
                } else {
                    gate
                }
            }),
            asks: true,
        };
        for (view, account) in [(true, false), (false, true)] {
            assert_eq!(
                policy(view, account).human_wall(&landing),
                Some(AgentBrowserHumanReason::SignIn)
            );
        }
        // A public read of the same page has nothing to sign in for.
        assert_eq!(policy(false, false).human_wall(&landing), None);
        // The signed-in app shows an account control and stays readable.
        let app = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"landmark","lm":"banner"}),
            json!({"k":3,"p":1,"r":"link","n":"Log in","o":9,"u":"https://example.test/login"}),
            json!({"k":4,"p":1,"r":"button","n":"Account","o":1,"ak":1}),
        ]);
        assert_eq!(policy(true, true).human_wall(&app), None);
    }

    fn keyboard_audit_action(
        observation: &SemanticObservation,
        key: SemanticPressKey,
        declared: SemanticEffectClass,
    ) -> SemanticPreparedAction {
        let snapshot = &observation.frames()[0];
        let proposal = SemanticActionProposal::try_new(
            SemanticActionIntent::Press {
                target: snapshot.nodes()[1].reference(),
                key,
            },
            declared,
            SemanticWaitCondition::Immediate,
            if snapshot.nodes()[1].value().is_some() {
                SemanticVerification::TargetValueChanged
            } else {
                SemanticVerification::PageChanged
            },
            SemanticSettleBudget::try_new(2000).unwrap(),
        )
        .unwrap();
        SemanticActionBatch::bind(
            SemanticActionBatchId::new(1).unwrap(),
            observation,
            &[snapshot.frame().clone()],
            vec![proposal],
        )
        .unwrap()
        .actions()[0]
            .prepare(snapshot)
            .unwrap()
    }

    #[test]
    fn keyboard_document_mutation_requires_the_same_edit_approval_as_fill() {
        let look = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"textbox","n":"Document body","o":11,"fs":1,"es":[1,2,false],"fc":true,
                "v":{"k":"text","value":"Draft"},"b":{"x":1,"y":1,"w":400,"h":60}}),
        ]);
        assert_eq!(
            effect(&look, 2, SemanticActionKind::Fill),
            SiteEffect::Commit(Consequence::Edit)
        );
        for key in [
            SemanticPressKey::Backspace,
            SemanticPressKey::Delete,
            SemanticPressKey::Space,
        ] {
            assert_eq!(
                effect_key(&look, 2, SemanticActionKind::Press, Some(key)),
                SiteEffect::Commit(Consequence::Edit),
                "{key:?}"
            );
        }
    }

    #[test]
    fn keyboard_document_mutation_cannot_declare_read_or_local_write_to_avoid_approval() {
        let look = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"textbox","n":"Document body","o":11,"fs":1,"es":[1,2,false],"fc":true,
                "v":{"k":"text","value":"Draft"},"b":{"x":1,"y":1,"w":400,"h":60}}),
        ]);
        for key in [
            SemanticPressKey::Backspace,
            SemanticPressKey::Delete,
            SemanticPressKey::Space,
        ] {
            for declared in [SemanticEffectClass::Read, SemanticEffectClass::LocalWrite] {
                let gate = Arc::new(SiteGate::new("example.test".into(), false, false));
                let policy = SiteWorkPolicy {
                    gate: gate.clone(),
                    asks: true,
                };
                assert!(
                    matches!(
                        policy.assess(&keyboard_audit_action(&look, key, declared), &look),
                        Err(AgentWorkFailure::ActionDenied)
                    ),
                    "{key:?} {declared:?}"
                );
                assert!(
                    gate.pending().is_none(),
                    "unknown caret cannot offer a one-step edit"
                );
            }
        }
    }

    #[test]
    fn keyboard_numeric_mutation_cannot_be_admitted_as_read() {
        let look = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"spinbutton","n":"Quantity","o":10,"fc":true,
                "v":{"k":"text","value":"17"},"b":{"x":1,"y":1,"w":200,"h":30}}),
        ]);
        let policy = SiteWorkPolicy {
            gate: Arc::new(SiteGate::default()),
            asks: true,
        };
        assert!(matches!(
            policy.assess(
                &keyboard_audit_action(&look, SemanticPressKey::ArrowUp, SemanticEffectClass::Read),
                &look
            ),
            Err(AgentWorkFailure::EffectRequired(
                SemanticEffectClass::LocalWrite
            ))
        ));
        assert!(policy
            .assess(
                &keyboard_audit_action(
                    &look,
                    SemanticPressKey::ArrowUp,
                    SemanticEffectClass::LocalWrite
                ),
                &look
            )
            .is_ok());
    }

    fn keyboard_editor_look() -> SemanticObservation {
        page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"textbox","n":"Document body","o":11,"fs":1,"es":[1,2,false],"fc":true,
                "v":{"k":"text","value":"Draft"},"b":{"x":1,"y":1,"w":400,"h":60}}),
        ])
    }

    #[test]
    fn keyboard_preview_names_the_actual_key_but_cannot_bind_unknown_selection() {
        let look = keyboard_editor_look();
        let snapshot = &look.frames()[0];
        let action = keyboard_audit_action(
            &look,
            SemanticPressKey::Backspace,
            SemanticEffectClass::ExternalWrite,
        );
        let node = &snapshot.nodes()[1];
        let shown = preview(Consequence::Edit, &action, node, snapshot, "example.test");
        assert_eq!(shown.action, "press Backspace in Document body");
        let gate = Arc::new(SiteGate::default());
        gate.state().pending = Some(Pending {
            fingerprint: fingerprint(&action, node, snapshot),
            digest: shown.digest(),
            preview: shown,
        });
        gate.approve(false).unwrap();
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        assert!(matches!(
            policy.assess(&action, &look),
            Err(AgentWorkFailure::ActionDenied)
        ));
        assert!(
            gate.state().confirmed.is_some(),
            "unbound selection must not consume one-step approval"
        );
        assert!(!gate.allow_edits());
        let allowed = SiteWorkPolicy {
            gate: Arc::new(SiteGate::new("example.test".into(), true, false)),
            asks: true,
        };
        assert!(allowed.assess(&action, &look).is_ok());
        assert!(matches!(
            allowed.assess(
                &keyboard_audit_action(
                    &look,
                    SemanticPressKey::Backspace,
                    SemanticEffectClass::LocalWrite
                ),
                &look
            ),
            Err(AgentWorkFailure::EffectRequired(
                SemanticEffectClass::ExternalWrite
            ))
        ));
    }

    #[test]
    fn keyboard_space_activation_keeps_checkbox_and_radio_consequences() {
        for role in ["checkbox", "radio"] {
            let look = page(vec![
                json!({"k":1,"r":"document","o":16,"fc":true}),
                json!({"k":2,"p":0,"r":role,"n":"Delete saved items","o":9,"fc":true,
                    "v":{"k":"boolean","value":false},"b":{"x":1,"y":1,"w":200,"h":30}}),
            ]);
            assert_eq!(
                effect_key(
                    &look,
                    2,
                    SemanticActionKind::Press,
                    Some(SemanticPressKey::Space)
                ),
                effect(&look, 2, SemanticActionKind::Click)
            );
            assert_eq!(
                effect(&look, 2, SemanticActionKind::Click),
                SiteEffect::Commit(Consequence::Destructive)
            );
        }
    }

    #[test]
    fn keyboard_consent_activation_cannot_accept_while_refusal_is_offered() {
        let look = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"button","n":"Accept all","o":9,"fc":true,"ak":1,"b":{"x":1,"y":1,"w":200,"h":30}}),
            json!({"k":3,"p":0,"r":"heading","n":"Cookies","l":1,"fc":true}),
            json!({"k":4,"p":0,"r":"paragraph","t":"We use cookies and other technologies.","fc":true}),
            json!({"k":5,"p":0,"r":"button","n":"Reject all","o":9,"fc":true,"ak":1,"b":{"x":1,"y":40,"w":200,"h":30}}),
        ]);
        let snapshot = &look.frames()[0];
        assert!(super::super::consent::offers_refusal(
            &snapshot.nodes()[1],
            snapshot
        ));
        let policy = SiteWorkPolicy {
            gate: Arc::new(SiteGate::default()),
            asks: true,
        };
        for key in [SemanticPressKey::Enter, SemanticPressKey::Space] {
            assert!(
                matches!(
                    policy.assess(
                        &keyboard_audit_action(&look, key, SemanticEffectClass::Read),
                        &look
                    ),
                    Err(AgentWorkFailure::ActionDenied)
                ),
                "{key:?}"
            );
        }
        assert!(
            policy
                .assess(
                    &keyboard_audit_action(
                        &look,
                        SemanticPressKey::ArrowDown,
                        SemanticEffectClass::Read
                    ),
                    &look
                )
                .is_ok(),
            "caret/navigation key is not consent activation"
        );
    }

    #[test]
    fn keyboard_search_draft_and_navigation_keep_their_context() {
        for (role, name, expected) in [
            ("searchbox", "Search", SiteEffect::Read),
            ("textbox", "Message to design", SiteEffect::Draft),
        ] {
            let look = page(vec![
                json!({"k":1,"r":"document","o":16,"fc":true}),
                json!({"k":2,"p":0,"r":role,"n":name,"o":10,"fc":true,"v":{"k":"text","value":"Draft"},"b":{"x":1,"y":1,"w":200,"h":30}}),
            ]);
            assert_eq!(
                effect_key(
                    &look,
                    2,
                    SemanticActionKind::Press,
                    Some(SemanticPressKey::Backspace)
                ),
                expected
            );
            let gate = Arc::new(SiteGate::default());
            let policy = SiteWorkPolicy {
                gate: gate.clone(),
                asks: true,
            };
            assert!(policy
                .assess(
                    &keyboard_audit_action(
                        &look,
                        SemanticPressKey::Backspace,
                        SemanticEffectClass::LocalWrite
                    ),
                    &look
                )
                .is_ok());
            assert!(gate.pending().is_none());
            if role == "searchbox" {
                assert!(
                    policy
                        .assess(
                            &keyboard_audit_action(
                                &look,
                                SemanticPressKey::Backspace,
                                SemanticEffectClass::Read
                            ),
                            &look
                        )
                        .is_ok(),
                    "search editing remains an intentional read"
                );
            }

            assert_eq!(
                effect_key(
                    &look,
                    2,
                    SemanticActionKind::Press,
                    Some(SemanticPressKey::ArrowLeft)
                ),
                SiteEffect::Read
            );
            let held = Arc::new(SiteGate::default().holding_typing(true));
            let policy = SiteWorkPolicy {
                gate: held.clone(),
                asks: true,
            };
            assert!(matches!(
                policy.assess(
                    &keyboard_audit_action(
                        &look,
                        SemanticPressKey::Backspace,
                        SemanticEffectClass::LocalWrite
                    ),
                    &look
                ),
                Err(AgentWorkFailure::ActionDenied)
            ));
            assert!(held.pending().is_none());
            held.state().allow_typing = true;
            assert!(policy
                .assess(
                    &keyboard_audit_action(
                        &look,
                        SemanticPressKey::Backspace,
                        SemanticEffectClass::LocalWrite
                    ),
                    &look
                )
                .is_ok());
        }
        let look = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"button","n":"Open details","o":9,"fc":true,"b":{"x":1,"y":1,"w":200,"h":30}}),
        ]);
        assert_eq!(
            effect(&look, 2, SemanticActionKind::Click),
            SiteEffect::Draft
        );
        let policy = SiteWorkPolicy {
            gate: Arc::new(SiteGate::default()),
            asks: true,
        };
        assert!(policy
            .assess(&click(&look, 2, SemanticEffectClass::Read), &look)
            .is_ok());
    }

    #[test]
    fn exact_approval_cannot_swap_to_an_identically_named_native_control() {
        let look = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"button","n":"Save changes","o":9,"ak":1,"fc":true,"b":{"x":1,"y":1,"w":100,"h":30}}),
            json!({"k":3,"p":0,"r":"button","n":"Save changes","o":9,"ak":1,"fc":true,"b":{"x":120,"y":1,"w":100,"h":30}}),
        ]);
        let first = click(&look, 2, SemanticEffectClass::ExternalWrite);
        let other = click(&look, 3, SemanticEffectClass::ExternalWrite);
        assert_ne!(first.target_reference(), other.target_reference());
        let gate = Arc::new(SiteGate::default());
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        assert!(matches!(
            policy.assess(&first, &look),
            Err(AgentWorkFailure::ActionDenied)
        ));
        gate.approve(false).unwrap();
        assert!(
            matches!(
                policy.assess(&other, &look),
                Err(AgentWorkFailure::ActionDenied)
            ),
            "approval of one control cannot authorize its identical-label sibling"
        );
    }

    #[test]
    fn exact_approval_requires_proof_and_cannot_cross_keyspace_reset() {
        let nodes = json!([
            {"k":1,"r":"document","o":16,"fc":true},
            {"k":2,"ai":1,"p":0,"r":"button","n":"Save changes","o":9,"ak":1,"fc":true,"b":{"x":1,"y":1,"w":100,"h":30}}
        ]);
        let capture = |nonce| {
            super::super::tests::reading_observation_with_approval(
                nodes.clone(),
                "complete",
                Some(nonce),
            )
        };
        let before = capture("000102030405060708090a0b0c0d0e0f");
        let reset = capture("100102030405060708090a0b0c0d0e0f");
        let gate = Arc::new(SiteGate::default());
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        let first = click(&before, 2, SemanticEffectClass::ExternalWrite);
        assert!(matches!(
            policy.assess(&first, &before),
            Err(AgentWorkFailure::ActionDenied)
        ));
        gate.approve(false).unwrap();
        let second = click(&reset, 2, SemanticEffectClass::ExternalWrite);
        assert!(matches!(
            policy.assess(&second, &reset),
            Err(AgentWorkFailure::ActionDenied)
        ));

        let mut legacy = nodes;
        legacy[1].as_object_mut().unwrap().remove("ai");
        let look = super::super::tests::reading_observation_with_approval(legacy, "complete", None);
        let gate = Arc::new(SiteGate::default());
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        assert!(matches!(
            policy.assess(&click(&look, 2, SemanticEffectClass::ExternalWrite), &look),
            Err(AgentWorkFailure::ActionDenied)
        ));
        assert!(
            gate.approve(false).is_none(),
            "missing native proof cannot offer an exact approval"
        );
    }

    fn approval_audit_checkbox(checked: bool) -> SemanticObservation {
        page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"checkbox","n":"Share publicly","o":9,"s":if checked {1}else{0},"fc":true,"v":{"k":"boolean","value":checked},"b":{"x":1,"y":1,"w":200,"h":30}}),
        ])
    }

    fn approval_audit_space(
        look: &SemanticObservation,
        verification: SemanticVerification,
    ) -> SemanticPreparedAction {
        let snapshot = &look.frames()[0];
        let proposal = SemanticActionProposal::try_new(
            SemanticActionIntent::Press {
                target: snapshot.nodes()[1].reference(),
                key: SemanticPressKey::Space,
            },
            SemanticEffectClass::Communication,
            SemanticWaitCondition::Immediate,
            verification,
            SemanticSettleBudget::try_new(2000).unwrap(),
        )
        .unwrap();
        SemanticActionBatch::bind(
            SemanticActionBatchId::new(1).unwrap(),
            look,
            &[snapshot.frame().clone()],
            vec![proposal],
        )
        .unwrap()
        .actions()[0]
            .prepare(snapshot)
            .unwrap()
    }

    #[test]
    fn exact_approval_cannot_weaken_verification_for_the_same_physical_key() {
        let look = approval_audit_checkbox(false);
        let exact = approval_audit_space(
            &look,
            SemanticVerification::TargetState {
                state: SemanticState::Checked,
                present: true,
            },
        );
        let weaker = approval_audit_space(&look, SemanticVerification::PageChanged);
        let gate = Arc::new(SiteGate::default());
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        assert!(matches!(
            policy.assess(&exact, &look),
            Err(AgentWorkFailure::ActionDenied)
        ));
        gate.approve(false).unwrap();
        assert!(
            matches!(
                policy.assess(&weaker, &look),
                Err(AgentWorkFailure::ActionDenied)
            ),
            "exact approval must bind its required verification"
        );
    }

    #[test]
    fn exact_approval_cannot_reverse_a_checkbox_edit_after_its_value_changes() {
        let before = approval_audit_checkbox(true);
        let after = approval_audit_checkbox(false);
        let first = approval_audit_space(&before, SemanticVerification::PageChanged);
        let changed = approval_audit_space(&after, SemanticVerification::PageChanged);
        let gate = Arc::new(SiteGate::default());
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        assert!(matches!(
            policy.assess(&first, &before),
            Err(AgentWorkFailure::ActionDenied)
        ));
        gate.approve(false).unwrap();
        assert!(
            matches!(
                policy.assess(&changed, &after),
                Err(AgentWorkFailure::ActionDenied)
            ),
            "approval to toggle checked off cannot authorize toggling unchecked on"
        );
    }

    #[test]
    fn approval_content_binds_hidden_value_suffix_but_allows_focus_change() {
        let look = |value: String, states: u8| {
            page(vec![
                json!({"k":1,"r":"document","o":16,"fc":true}),
                json!({"k":2,"p":0,"r":"textbox","n":"Message to design","o":10,"s":states,"fc":true,"v":{"k":"text","value":value},"b":{"x":1,"y":1,"w":200,"h":30}}),
            ])
        };
        let prefix = "x".repeat(zephium_agentic::MAX_SEMANTIC_VALUE_PREVIEW_BYTES + 100);
        let before = look(format!("{prefix}A"), 0);
        let changed = look(format!("{prefix}B"), 0);
        let focused = look(format!("{prefix}A"), 64);
        let a = keyboard_audit_action(
            &before,
            SemanticPressKey::Enter,
            SemanticEffectClass::Communication,
        );
        let snapshot = &before.frames()[0];
        let source = &snapshot.nodes()[1];
        assert_eq!(
            value_text(source),
            value_text(&changed.frames()[0].nodes()[1]),
            "public previews intentionally omit the distinct tail"
        );
        assert_ne!(
            fingerprint(&a, source, snapshot),
            fingerprint(&a, &changed.frames()[0].nodes()[1], &changed.frames()[0])
        );
        assert_eq!(
            fingerprint(&a, source, snapshot),
            fingerprint(&a, &focused.frames()[0].nodes()[1], &focused.frames()[0])
        );
    }

    fn composer(buttons: &[(u64, &str)]) -> SemanticObservation {
        let mut nodes = vec![json!({"k":1,"r":"document","o":16,"fc":true})];
        nodes.extend(buttons.iter().map(|(key, name)| {
            json!({"k":key,"p":0,"r":"button","n":name,"o":9,"ak":1,"fc":true,
                "b":{"x":1,"y":1,"w":100,"h":30}})
        }));
        page(nodes)
    }

    #[test]
    fn an_approval_follows_its_step_to_the_control_the_page_renders_anew() {
        let before = composer(&[(2, "Send")]);
        let gate = Arc::new(SiteGate::default());
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        let held = click(&before, 2, SemanticEffectClass::Communication);
        assert!(matches!(
            policy.assess(&held, &before),
            Err(AgentWorkFailure::ActionDenied)
        ));
        gate.approve(false).unwrap();
        // The single-page app replaced the node while the card waited.
        let after = page(vec![
            json!({"k":1,"r":"document","o":16,"fc":true}),
            json!({"k":2,"p":0,"r":"paragraph","t":"Sending","fc":true}),
            json!({"k":3,"p":0,"r":"button","n":"Send","o":9,"ak":1,"fc":true,
                "b":{"x":1,"y":1,"w":100,"h":30}}),
        ]);
        let renewed = click(&after, 3, SemanticEffectClass::Communication);
        assert_ne!(held.approval_binding(), renewed.approval_binding());
        assert!(policy.assess(&renewed, &after).is_ok());
        policy.after_step(&after);
        assert_eq!(gate.take_receipt(), Some(Receipt::Committed));
        assert_eq!(gate.committed(), vec!["Sent on the page: Send".to_owned()]);
    }

    #[test]
    fn an_approved_step_that_two_controls_could_be_runs_when_proposed_again_without_a_new_card() {
        let before = composer(&[(2, "Send")]);
        let gate = Arc::new(SiteGate::default());
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        assert!(policy
            .assess(
                &click(&before, 2, SemanticEffectClass::Communication),
                &before
            )
            .is_err());
        gate.approve(false).unwrap();
        let twins = composer(&[(2, "Send"), (3, "Send")]);
        let again = click(&twins, 3, SemanticEffectClass::Communication);
        assert!(matches!(
            policy.assess(&again, &twins),
            Err(AgentWorkFailure::ActionDenied)
        ));
        assert_eq!(gate.take_receipt(), Some(Receipt::NotSent));
        assert!(gate.pending().is_none(), "the person is not asked twice");
        assert!(policy.assess(&again, &twins).is_ok());
        assert!(matches!(
            policy.assess(&again, &twins),
            Err(AgentWorkFailure::ActionDenied)
        ));
    }

    #[test]
    fn removing_a_value_from_an_editor_being_filled_edits_the_draft() {
        let editor = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"dialog","n":"Add task"}),
            json!({"k":3,"p":1,"r":"textbox","n":"Task name","o":10,"v":{"k":"text","value":"Pay rent"}}),
            json!({"k":4,"p":1,"r":"button","n":"Remove date","o":1,"ak":1}),
            json!({"k":5,"p":1,"r":"button","n":"Discard draft","o":1,"ak":1}),
            json!({"k":6,"p":1,"r":"button","n":"Add task","o":1,"ak":1}),
        ]);
        assert_eq!(
            effect(&editor, 4, SemanticActionKind::Click),
            SiteEffect::Draft
        );
        assert_eq!(
            effect(&editor, 5, SemanticActionKind::Click),
            SiteEffect::Commit(Consequence::Destructive)
        );
        let saved = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"dialog","n":"Task"}),
            json!({"k":3,"p":1,"r":"textbox","n":"Task name","o":10,"v":{"k":"text","value":"Pay rent"}}),
            json!({"k":4,"p":1,"r":"button","n":"Delete task","o":1,"ak":1}),
            json!({"k":5,"p":1,"r":"button","n":"Remove date","o":1,"ak":1}),
        ]);
        assert_eq!(
            effect(&saved, 4, SemanticActionKind::Click),
            SiteEffect::Commit(Consequence::Destructive)
        );
        assert_eq!(
            effect(&saved, 5, SemanticActionKind::Click),
            SiteEffect::Commit(Consequence::Destructive),
            "nothing unsaved to edit: the control removes what the page keeps"
        );
        let list = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"list","n":"Messages"}),
            json!({"k":3,"p":1,"r":"button","n":"Delete","o":1,"ak":1}),
            json!({"k":4,"p":0,"r":"button","n":"Remove from cart","o":1,"ak":1}),
        ]);
        for key in [3, 4] {
            assert_eq!(
                effect(&list, key, SemanticActionKind::Click),
                SiteEffect::Commit(Consequence::Destructive)
            );
        }
    }

    #[test]
    fn an_approved_step_the_page_refused_before_it_ran_runs_again_without_a_new_card() {
        let look = composer(&[(2, "Send")]);
        let gate = Arc::new(SiteGate::default());
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        let send = click(&look, 2, SemanticEffectClass::Communication);
        assert!(policy.assess(&send, &look).is_err());
        gate.approve(false).unwrap();
        assert!(policy.assess(&send, &look).is_ok());
        policy.action_not_run();
        assert_eq!(gate.take_receipt(), None, "nothing ran, nothing to report");
        assert_eq!(gate.finish(), Some(Receipt::NotSent));
        assert!(policy.assess(&send, &look).is_ok());
        assert!(gate.pending().is_none(), "the person is not asked twice");
        policy.after_step(&look);
        assert_eq!(gate.take_receipt(), Some(Receipt::Committed));
        assert_eq!(gate.committed(), vec!["Sent on the page: Send".to_owned()]);
    }

    #[test]
    fn a_commit_whose_outcome_the_page_did_not_show_never_runs_again() {
        let look = composer(&[(2, "Send")]);
        let gate = Arc::new(SiteGate::default());
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        let send = click(&look, 2, SemanticEffectClass::Communication);
        assert!(policy.assess(&send, &look).is_err());
        gate.approve(false).unwrap();
        assert!(policy.assess(&send, &look).is_ok());
        policy.action_outcome_unknown();
        assert_eq!(gate.take_receipt(), Some(Receipt::Unverified));
        assert_eq!(
            policy.assess(&send, &look).err(),
            Some(AgentWorkFailure::OutcomeUnknown)
        );
        assert!(
            gate.pending().is_none(),
            "no card for a step that may have run"
        );
        assert_eq!(
            gate.committed(),
            vec!["Maybe sent on the page, not confirmed: Send".to_owned()]
        );
        // The same control re-rendered is the same step.
        let renewed = composer(&[(3, "Send")]);
        assert_eq!(
            policy
                .assess(
                    &click(&renewed, 2, SemanticEffectClass::Communication),
                    &renewed
                )
                .err(),
            Some(AgentWorkFailure::OutcomeUnknown)
        );
    }

    #[test]
    fn an_add_to_cart_the_page_refused_before_it_ran_is_not_counted() {
        let look = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"heading","l":1,"n":"Combination Pliers"}),
            json!({"k":3,"p":0,"r":"button","n":"Add to cart","o":1,"ak":1,"b":{"x":1,"y":1,"w":80,"h":30}}),
        ]);
        let policy = SiteWorkPolicy {
            gate: Arc::new(SiteGate::new("shop.example".into(), true, false)),
            asks: true,
        };
        let add = click(&look, 3, SemanticEffectClass::ExternalWrite);
        assert!(policy.assess(&add, &look).is_ok());
        policy.action_not_run();
        assert!(policy.assess(&add, &look).is_ok());
        policy.action_outcome_unknown();
        assert_eq!(
            policy.assess(&add, &look).err(),
            Some(AgentWorkFailure::OutcomeUnknown)
        );
    }

    #[test]
    fn the_ledger_names_what_the_page_task_committed_in_order() {
        let gate = Arc::new(SiteGate::new("example.com".into(), false, false));
        let policy = SiteWorkPolicy {
            gate: gate.clone(),
            asks: true,
        };
        assert!(gate.committed().is_empty());
        let observation = booking("$1,240.00");
        let action = click(&observation, 6, SemanticEffectClass::Purchase);
        assert!(policy.assess(&action, &observation).is_err());
        assert!(gate.committed().is_empty(), "a held step committed nothing");
        gate.approve(false).unwrap();
        assert!(policy.assess(&action, &observation).is_ok());
        policy.after_step(&observation);
        assert_eq!(
            gate.committed(),
            vec!["Purchased on the page: Request to book".to_owned()]
        );
    }

    #[test]
    fn setting_where_to_deliver_is_a_draft_never_a_commit() {
        let location = page(vec![
            json!({"k":1,"r":"document","o":16}),
            json!({"k":2,"p":0,"r":"dialog","n":"Choose your location"}),
            json!({"k":3,"p":1,"r":"paragraph","t":"Delivery options and speeds vary by location"}),
            json!({"k":4,"p":1,"r":"textbox","n":"Enter a US zip code","o":10,"v":{"k":"text","value":"10001"}}),
            json!({"k":5,"p":1,"r":"button","n":"Apply","o":1,"ak":1}),
            json!({"k":6,"p":1,"r":"button","n":"Done","o":1,"ak":1}),
            json!({"k":7,"p":1,"r":"button","n":"Continue","o":1,"ak":1}),
        ]);
        for key in [5, 6, 7] {
            assert!(
                matches!(
                    effect(&location, key, SemanticActionKind::Click),
                    SiteEffect::Draft | SiteEffect::Read
                ),
                "{key}"
            );
        }
        assert!(matches!(
            effect(&location, 4, SemanticActionKind::Fill),
            SiteEffect::Draft | SiteEffect::Read
        ));
    }
}
