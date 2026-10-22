//! Mechanical pins for the Ask panel's Browse reader: the document-start
//! piece stays small and closed, Rust is the only source of program bytes,
//! nothing evaluates script through an activation-granting API, recipes are
//! closed invocations, and the reader never reaches private windows.

use std::path::Path;

const BOOTSTRAP: &str = "crates/zephium-engine/src/platform/page_reader_bootstrap.js";
const RECIPES: &str = "crates/zephium-engine/src/platform/page_reader_recipes.js";
const SERVE: &str = "crates/zephium-engine/src/platform/macos/page_reader_serve.js";
const RECIPE: &str = "crates/zephium-engine/src/platform/page_recipe.rs";
const MACOS_READER: &str = "crates/zephium-engine/src/platform/macos/browse_reader.rs";
const WINDOWS_READER: &str = "crates/zephium-engine/src/platform/windows/browse_reader.rs";
const PROTOCOL: &str = "crates/zephium-engine/src/platform/agent_semantic_cdp_protocol.rs";
const HOST: &str = "crates/zephium-engine/src/host/page_reader.rs";
const CONSTRUCTION: &str = "crates/zephium-engine/src/host/construction.rs";
const PUBLIC: &str = "crates/zephium-engine/src/page_reader.rs";
const RUNTIME: &str = "crates/zephium-engine/src/platform/macos/semantic_runtime.rs";
const ENGINE_MANIFEST: &str = "crates/zephium-engine/Cargo.toml";
const SHIPPING_MANIFESTS: [&str; 2] = ["desktop/Cargo.toml", "crates/zephium-app/Cargo.toml"];
const MAX_BOOTSTRAP_BYTES: usize = 1_024;

const NATIVE_HEADER: [&str; 4] = [
    "#![deny(unsafe_op_in_unsafe_fn)]",
    "#![deny(clippy::undocumented_unsafe_blocks)]",
    "#![deny(clippy::dbg_macro,clippy::print_stderr,clippy::print_stdout)]",
    "#![cfg_attr(not(test),deny(clippy::panic,clippy::unreachable,clippy::unwrap_used))]",
];

/// Ways to run script that would confer user activation, or to run it in
/// the page's own world.
const EVALUATION: [&str; 8] = [
    "evaluateJavaScript",
    "callAsyncJavaScript",
    "evaluate_script",
    "ExecuteScript",
    "pageWorld",
    "defaultClientWorld",
    "\"userGesture\":true",
    "with_ipc_handler",
];

/// What the in-page pieces may never do: run strings, reach the network,
/// write markup, move the person, or act for them.
const PAGE_FORBIDDEN: [&str; 22] = [
    "eval(",
    "new Function",
    "fetch(",
    "XMLHttpRequest",
    "WebSocket",
    "sendBeacon",
    "import(",
    "innerHTML",
    "outerHTML",
    "insertAdjacentHTML",
    "document.write",
    "createElement",
    "setAttribute",
    "location.",
    "window.open",
    ".click(",
    "dispatchEvent",
    ".focus(",
    ".submit(",
    "localStorage",
    "indexedDB",
    "cookie",
];

pub(crate) fn check(repository: &Path) -> Result<(), String> {
    check_bootstrap(&read(repository, BOOTSTRAP)?)?;
    check_recipes(&read(repository, RECIPES)?)?;
    check_serve(&read(repository, SERVE)?)?;
    check_recipe_grammar(&read(repository, RECIPE)?)?;
    check_macos_reader(&read(repository, MACOS_READER)?)?;
    check_windows_reader(
        &read(repository, WINDOWS_READER)?,
        &read(repository, PROTOCOL)?,
    )?;
    check_host(&read(repository, HOST)?, &read(repository, CONSTRUCTION)?)?;
    check_public(&read(repository, PUBLIC)?)?;
    check_runtime_recipe(&read(repository, RUNTIME)?)?;
    check_probe_gate(&read(repository, ENGINE_MANIFEST)?)?;
    for manifest in SHIPPING_MANIFESTS {
        if read(repository, manifest)?.contains("page-reader-probe") {
            return Err(format!("{manifest} reaches the page reader probe"));
        }
    }
    Ok(())
}

fn check_bootstrap(source: &str) -> Result<(), String> {
    if source.len() > MAX_BOOTSTRAP_BYTES || !source.is_ascii() {
        return Err(format!(
            "the document-start piece grew past {MAX_BOOTSTRAP_BYTES} bytes or left ASCII"
        ));
    }
    let compact = compact(source);
    for required in [
        "messageHandlers.zephiumPageAssistBootV1",
        "run=Function",
        "serve=run(a)()",
        "[serve===null?\"B1\":\"B2\"]",
        "a===\"S1\"",
        "a!==\"G1\"",
    ] {
        if compact.matches(required).count() != 1 {
            return Err(format!("the document-start piece lost {required}"));
        }
    }
    // The only program it runs is the reply to its own pull, on its own
    // private-world handler: never page text, never a second source.
    if compact.matches("run(").count() != 1 || compact.matches("Function").count() != 1 {
        return Err("the document-start piece runs a second source".to_owned());
    }
    for forbidden in PAGE_FORBIDDEN
        .iter()
        .chain(["document", "setTimeout", "setInterval"].iter())
    {
        if source.contains(forbidden) {
            return Err(format!("the document-start piece acquired {forbidden}"));
        }
    }
    Ok(())
}

fn check_recipes(source: &str) -> Result<(), String> {
    let compact = compact(source);
    for required in [
        "constNAME=\"__zephiumPageAssistV1\";if(Object.hasOwn(globalThis,NAME))return;",
        "writable:false,configurable:false,enumerable:false",
        "if(encoded===\"Q1:selection\")returnselection();",
        "if(encoded===\"Q1:outline\")returnoutline();",
        "encoded.startsWith(\"Q1:mark:\"))returnmarkQuote(encoded.slice(8));",
        "try{quote=parse(encoded);}",
        "if(protectedField(active))return\"N\";",
        "MAX_NODES=48",
    ] {
        if !compact.contains(required) {
            return Err(format!("the recipes lost {required}"));
        }
    }
    if compact.matches("\"Q1:").count() != 3 {
        return Err("the recipes gained an invocation".to_owned());
    }
    for forbidden in PAGE_FORBIDDEN
        .iter()
        .chain(["Function(", "postMessage"].iter())
    {
        if source.contains(forbidden) {
            return Err(format!("the recipes acquired {forbidden}"));
        }
    }
    Ok(())
}

fn check_serve(source: &str) -> Result<(), String> {
    let compact = compact(source);
    for required in [
        "handlers.zephiumPageAssistV1",
        "[\"P1\"]",
        "request===\"S1\")return;",
        "request.startsWith(\"Q1:\")?zephiumAssist.recipe(request):awaitzephiumRuntime.invoke(request)",
        "[\"R1:\"+result]",
        "if(acknowledgement!==\"A1\")return;",
    ] {
        if !compact.contains(required) {
            return Err(format!("the reader's pull loop lost {required}"));
        }
    }
    for forbidden in PAGE_FORBIDDEN
        .iter()
        .chain(["Function(", "document"].iter())
    {
        if source.contains(forbidden) {
            return Err(format!("the reader's pull loop acquired {forbidden}"));
        }
    }
    Ok(())
}

fn check_recipe_grammar(source: &str) -> Result<(), String> {
    let compact = compact(production(source));
    for required in [
        "constSELECTION:&str=\"Q1:selection\";",
        "constOUTLINE:&str=\"Q1:outline\";",
        "constMARK_PREFIX:&str=\"Q1:mark:\";",
        "letjson=serde_json::to_string(quote).ok()?;",
        "encoded:format!(\"{MARK_PREFIX}{json}\"),",
        "Kind::Selection=>MAX_SELECTION_WIRE_BYTES,",
        "Kind::Outline=>MAX_OUTLINE_WIRE_BYTES,",
        "Kind::Mark=>1,",
        "ifbytes.len()>limit{returnNone;}",
    ] {
        if !compact.contains(required) {
            return Err(format!("the recipe grammar lost {required}"));
        }
    }
    if compact.matches("Self{kind:Kind::").count() != 3 {
        return Err("the recipe grammar gained an invocation".to_owned());
    }
    Ok(())
}

fn check_macos_reader(source: &str) -> Result<(), String> {
    let whole = compact(source);
    for header in NATIVE_HEADER {
        if !whole.contains(header) {
            return Err(format!("the macOS reader lost {header}"));
        }
    }
    let production = compact(production(source));
    for required in [
        "WKUserScriptInjectionTime::AtDocumentStart,true,&self.world,",
        "WKContentWorld::worldWithName(&name,mtm)",
        "format!(\"{WORLD_PREFIX}{:016x}{:016x}\",high.finish(),low.finish())",
        "_removeAllUserScriptsAssociatedWithContentWorld",
        "respondsToSelector:sel!(_removeAllUserScriptsAssociatedWithContentWorld:)",
        "browse_channel(",
        "letmessage=ifpull.instantiated{RESTART}else{program()};",
        "program.push_str(installation);",
        "program.push_str(RECIPES);",
        "program.push_str(SERVE);",
        "letinstallation=zephium_agentic::SEMANTIC_RUNTIME_PROGRAM.installation_source();",
        "message.world()",
        "message.name()",
        "message.webView()",
        "message.frameInfo()",
        "frame.isMainFrame()",
        "Retained::as_ptr(&world)!=Retained::as_ptr(&ivars.world)",
        "!same_view(webview.as_deref())",
        "!same_view(frame_webview.as_deref())",
        "body.lengthOfBytesUsingEncoding(NSUTF8StringEncoding)>2",
        "![BOOT_FRESH,BOOT_INSTANTIATED,BOOT_FAILED].contains(&body.as_str())",
        "ifself.channel.as_ref().is_some_and(|channel|channel.epoch!=epoch){self.stop_channel();}",
        "if(self.committed_now)()!=Some(epoch)||self.committed!=Some(epoch){op.fail(PageReadFailure::PageChanged);",
        "Ok(_)ifcommitted_now()!=Some(epoch)=>{Err(PageReadFailure::PageChanged)}",
        "done(answer.filter(|_|committed_now()==Some(epoch)));",
    ] {
        if !production.contains(required) {
            return Err(format!("the macOS reader lost {required}"));
        }
    }
    // Native answers a pull only with a fixed token or the fixed program.
    let answers = production.matches("answer(").count();
    if answers != 7 {
        return Err(format!(
            "the macOS reader answers pulls from {answers} places; review each before re-pinning"
        ));
    }
    for answer in [
        "answer(reply,STOP)",
        "answer(pull.reply,STOP)",
        "answer(pull.reply,message)",
    ] {
        if !production.contains(answer) {
            return Err(format!("the macOS reader lost {answer}"));
        }
    }
    if production.matches("include_str!(").count() != 3 {
        return Err("the macOS reader gained a program source".to_owned());
    }
    for forbidden in EVALUATION
        .iter()
        .chain(["std::env", "println!", "eprintln!"].iter())
    {
        if production.contains(forbidden) {
            return Err(format!("the macOS reader acquired {forbidden}"));
        }
    }
    Ok(())
}

fn check_windows_reader(source: &str, protocol: &str) -> Result<(), String> {
    let whole = compact(source);
    for header in NATIVE_HEADER {
        if !whole.contains(header) {
            return Err(format!("the Windows reader lost {header}"));
        }
    }
    let production = compact(production(source));
    for required in [
        "letunpredictable=windows_core::GUID::new().ok()?.to_u128();",
        "SemanticWorldName::from_nonce(epoch,unpredictable).ok()",
        "self.world=next_world_name();",
        "discovery.create_world_command()",
        "install_runtime_in_context_command(&context)",
        "install_page_recipes_in_context_command(context)",
        "Self::Read{invocation,..}=>invoke_runtime_command(context,invocation).ok(),",
        "Self::Recipe{recipe,..}=>invoke_page_recipe_command(context,recipe).ok(),",
        "ifself.in_flight.is_some(){",
        "if(self.committed_now)()!=Some(epoch)||self.committed!=Some(epoch){op.fail(PageReadFailure::PageChanged);",
        "if(self.committed_now)()!=Some(current.epoch){returncurrent.serving.fail(PageReadFailure::PageChanged);}",
        "self.fail_current(PageReadFailure::PageChanged);self.forget_document();",
    ] {
        if !production.contains(required) {
            return Err(format!("the Windows reader lost {required}"));
        }
    }
    // Every CDP byte comes from the closed protocol module.
    for forbidden in EVALUATION.iter().chain(
        [
            "json!",
            "\"userGesture\"",
            "AddScriptToExecuteOnDocumentCreated",
            "PostWebMessage",
            "Runtime.evaluate",
            "std::env",
            "println!",
            "eprintln!",
        ]
        .iter(),
    ) {
        if production.contains(forbidden) {
            return Err(format!("the Windows reader acquired {forbidden}"));
        }
    }
    let protocol = compact(protocol);
    for required in [
        "constPAGE_RECIPES_SOURCE:&str=include_str!(\"page_reader_recipes.js\");",
        "constPAGE_RECIPE_FUNCTION:&str=\"function(encoded){returnglobalThis.__zephiumPageAssistV1.recipe(encoded);}\";",
        "\"arguments\":[{\"value\":recipe.as_str()}],",
        ".and_then(|value|recipe.decode(value.as_bytes()))",
    ] {
        if !protocol.contains(required) {
            return Err(format!("the Windows recipe protocol lost {required}"));
        }
    }
    if protocol.contains("\"userGesture\":true") {
        return Err("the Windows recipe protocol grants user activation".to_owned());
    }
    Ok(())
}

fn check_host(host: &str, construction: &str) -> Result<(), String> {
    let host = compact(production(host));
    for required in [
        "policy().is_some_and(|policy|!(policy.sensitive)(target))",
        "if!admits(&url){returnErr(PageReadFailure::Unavailable);}",
        "ifcommitted.presentation_id().into_raw()!=epoch{returnErr(PageReadFailure::PageChanged);}",
    ] {
        if !host.contains(required) {
            return Err(format!("the reader host lost {required}"));
        }
    }
    let construction = compact(construction);
    if !construction.contains(
        "if!matches!(partition,Partition::Ephemeral(_)){letcommitted=navigation.clone();*page_reader.borrow_mut()=",
    ) {
        return Err("a private window could receive the page reader".to_owned());
    }
    if construction.matches("BrowseReader::new(").count() != 2
        || construction
            .matches("if!matches!(partition,Partition::Ephemeral(_)){letcommitted=navigation.clone();*page_reader.borrow_mut()=")
            .count()
            != 2
    {
        return Err("the page reader gained a construction path".to_owned());
    }
    Ok(())
}

fn check_public(source: &str) -> Result<(), String> {
    let compact = compact(production(source));
    for required in [
        "SemanticReadSensitivityLimit::PublicOnly",
        "SemanticReadBudget::STANDARD",
        "verbatim:fragment.verbatim_text().is_some()",
        "iffield&&!protected=>",
    ] {
        if !compact.contains(required) {
            return Err(format!("the read projection lost {required}"));
        }
    }
    if compact.contains("SemanticReadSensitivityLimit::Sensitive") {
        return Err("the read projection widened past public content".to_owned());
    }
    Ok(())
}

fn check_runtime_recipe(source: &str) -> Result<(), String> {
    let compact = compact(source);
    for required in [
        "Recipe{recipe:crate::platform::page_recipe::PageRecipe,completion:RecipeCompletion,},",
        "Self::Recipe{recipe,..}=>recipe.as_str(),",
        "PendingInvocation::Recipe{recipe,completion}=>(CompletionAction::Recipe{completion,outcome:(self.phase==DocumentPhase::Ready).then(||recipe.decode(bytes)).flatten(),},true,),",
        "pub(crate)fnbrowse_channel(",
    ] {
        if !compact.contains(required) {
            return Err(format!("the pinned runtime lost the reader's closed recipe {required}"));
        }
    }
    Ok(())
}

fn check_probe_gate(manifest: &str) -> Result<(), String> {
    let compact = compact(manifest);
    for required in [
        "page-reader-probe=[\"agentic-browser\"]",
        "name=\"macos-page-reader-probe\"path=\"src/bin/macos_page_reader_probe.rs\"required-features=[\"page-reader-probe\"]",
    ] {
        if !compact.contains(required) {
            return Err(format!("the page reader probe left its feature gate: {required}"));
        }
    }
    Ok(())
}

fn production(source: &str) -> &str {
    source
        .find("#[cfg(test)]\nmod tests")
        .map_or(source, |start| &source[..start])
}

fn compact(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

fn read(repository: &Path, path: &str) -> Result<String, String> {
    std::fs::read_to_string(repository.join(path))
        .map_err(|error| format!("cannot read {path}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_passes() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        check(&repository).expect("page reader boundary");
    }

    #[test]
    fn a_bigger_or_eval_bootstrap_is_refused() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let bootstrap = read(&repository, BOOTSTRAP).expect("bootstrap");
        assert!(check_bootstrap(&bootstrap).is_ok());
        assert!(
            check_bootstrap(&format!("{bootstrap}{}", " ".repeat(MAX_BOOTSTRAP_BYTES))).is_err()
        );
        assert!(check_bootstrap(
            &bootstrap.replace("pull();})();", "eval(location.hash);pull();})();")
        )
        .is_err());
        let recipes = read(&repository, RECIPES).expect("recipes");
        assert!(check_recipes(&recipes).is_ok());
        assert!(check_recipes(&format!("{recipes}\nfetch(\"x\")")).is_err());
        let reader = read(&repository, MACOS_READER).expect("reader");
        assert!(check_macos_reader(&reader).is_ok());
        assert!(
            check_macos_reader(&reader.replace("fn title(", "fn evaluateJavaScript(")).is_err()
        );
    }
}
